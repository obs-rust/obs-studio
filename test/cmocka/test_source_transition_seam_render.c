/*
 * The transition seam's render, audio and lifecycle accessors
 * (libobs/obs-source-transition-seam.h, obs-rust/obs-studio#4074), which the
 * ports of the transition video render and of the transition audio render,
 * init and free call in place of the struct reads and calls they wrap:
 * - obs_source_transition_seam_get_context_data(): the transition's
 *   context.data;
 * - obs_source_transition_seam_get_transparent_texture() and
 *   _get_graphics(): the core's obs->video.transparent_texture and
 *   obs->video.graphics;
 * - obs_source_transition_seam_get_child_audio_pending(),
 *   _get_child_audio_is_duplicated() and _get_child_audio_output_buf(): a
 *   child's audio_pending, audio_is_duplicated and audio_output_buf[0][0];
 * - obs_source_transition_seam_init_mutex_values(), _mutex_init(),
 *   _tex_mutex_init(), _mutex_destroy() and _tex_mutex_destroy(): the
 *   pthread_mutex_init_value(), pthread_mutex_init() and
 *   pthread_mutex_destroy() calls of obs_transition_init() and
 *   obs_transition_free() on transition_mutex and transition_tex_mutex.
 *
 * The accessors are hidden in libobs, so this test compiles the seam (and
 * the load checks its transition_valid() asks) into itself, and runs them
 * on a stand-in core (`obs`, with only its graphics and transparent texture
 * pointers) and stand-in sources: zeroed struct obs_source with only the
 * fields each accessor reads. The pointers it reads are tags, never
 * dereferenced: no graphics subsystem is needed.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <string.h>

#include "obs-internal.h"
#include "obs-source-transition-seam.h"

/* libobs has this one in obs.c, hidden; the seam in this test reads this
 * stand-in. */
struct obs_core *obs;

static int data_tag;
static int other_data_tag;
static int graphics_tag;
static int texture_tag;

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	obs = bzalloc(sizeof(*obs));
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	bfree(obs);
	obs = NULL;
	return 0;
}

/* The context data as it is, each time: the pointer the type's create
 * returned, another one, or NULL. */
static void context_data_is_the_field(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *tr = bzalloc(sizeof(*tr));

	assert_null(obs_source_transition_seam_get_context_data(tr));

	tr->context.data = &data_tag;
	assert_ptr_equal(obs_source_transition_seam_get_context_data(tr), &data_tag);

	tr->context.data = &other_data_tag;
	assert_ptr_equal(obs_source_transition_seam_get_context_data(tr), &other_data_tag);

	bfree(tr);
}

/* The core's graphics and transparent texture, each its own field: NULL
 * before they are set, then what was set, and neither follows the other. */
static void core_reads_are_the_fields(void **state)
{
	UNUSED_PARAMETER(state);
	obs->video.graphics = NULL;
	obs->video.transparent_texture = NULL;
	assert_null(obs_source_transition_seam_get_graphics());
	assert_null(obs_source_transition_seam_get_transparent_texture());

	obs->video.graphics = (graphics_t *)&graphics_tag;
	assert_ptr_equal(obs_source_transition_seam_get_graphics(), &graphics_tag);
	assert_null(obs_source_transition_seam_get_transparent_texture());

	obs->video.transparent_texture = (gs_texture_t *)&texture_tag;
	assert_ptr_equal(obs_source_transition_seam_get_transparent_texture(), &texture_tag);
	assert_ptr_equal(obs_source_transition_seam_get_graphics(), &graphics_tag);

	obs->video.graphics = NULL;
	assert_null(obs_source_transition_seam_get_graphics());
	assert_ptr_equal(obs_source_transition_seam_get_transparent_texture(), &texture_tag);

	obs->video.transparent_texture = NULL;
}

/* A child's two audio flags, each its own field, in all four
 * combinations. */
static void child_audio_flags_are_the_fields(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *child = bzalloc(sizeof(*child));

	for (int pending = 0; pending < 2; pending++) {
		for (int duplicated = 0; duplicated < 2; duplicated++) {
			child->audio_pending = pending != 0;
			child->audio_is_duplicated = duplicated != 0;
			assert_int_equal(obs_source_transition_seam_get_child_audio_pending(child), pending != 0);
			assert_int_equal(obs_source_transition_seam_get_child_audio_is_duplicated(child),
					 duplicated != 0);
		}
	}

	bfree(child);
}

/* A child's first output buffer, the start of the one block its buffers
 * share (as obs-source.c allocates them): mix 0, channel 0, and no other
 * mix or channel. */
static void child_audio_output_buf_is_the_first(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *child = bzalloc(sizeof(*child));
	size_t frames = AUDIO_OUTPUT_FRAMES;
	float *block = bzalloc(sizeof(float) * frames * MAX_AUDIO_CHANNELS * MAX_AUDIO_MIXES);

	assert_null(obs_source_transition_seam_get_child_audio_output_buf(child));

	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++)
			child->audio_output_buf[mix][ch] = block + (mix * MAX_AUDIO_CHANNELS + ch) * frames;
	}
	assert_ptr_equal(obs_source_transition_seam_get_child_audio_output_buf(child), block);

	/* Only [0][0] counts: moving any other buffer changes nothing. */
	child->audio_output_buf[0][1] = NULL;
	child->audio_output_buf[1][0] = NULL;
	assert_ptr_equal(obs_source_transition_seam_get_child_audio_output_buf(child), block);

	child->audio_output_buf[0][0] = block + frames;
	assert_ptr_equal(obs_source_transition_seam_get_child_audio_output_buf(child), block + frames);

	bfree(block);
	bfree(child);
}

/* The init values: both mutexes become PTHREAD_MUTEX_INITIALIZER, and
 * nothing else of the source changes. */
static void init_mutex_values_sets_both(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *tr = bzalloc(sizeof(*tr));
	obs_source_t *expected = bzalloc(sizeof(*expected));

	memset(tr, 0xa5, sizeof(*tr));
	memcpy(expected, tr, sizeof(*tr));
	pthread_mutex_init_value(&expected->transition_mutex);
	pthread_mutex_init_value(&expected->transition_tex_mutex);

	obs_source_transition_seam_init_mutex_values(tr);
	assert_memory_equal(tr, expected, sizeof(*tr));

	bfree(expected);
	bfree(tr);
}

/* Init, destroy and init again, as obs_transition_init() and
 * obs_transition_free() do: each init returns 0 and leaves its mutex
 * usable, and each call works on its own mutex only. */
static void mutex_init_destroy_init(void **state)
{
	UNUSED_PARAMETER(state);
	obs_source_t *tr = bzalloc(sizeof(*tr));
	pthread_mutex_t tex_before;
	pthread_mutex_t mutex_before;

	obs_source_transition_seam_init_mutex_values(tr);
	assert_int_equal(obs_source_transition_seam_mutex_init(tr), 0);
	assert_int_equal(obs_source_transition_seam_tex_mutex_init(tr), 0);

	assert_int_equal(obs_source_transition_seam_trylock(tr), 0);
	assert_int_equal(obs_source_transition_seam_trylock_textures(tr), 0);
	assert_int_equal(obs_source_transition_seam_unlock_textures(tr), 0);
	assert_int_equal(obs_source_transition_seam_unlock(tr), 0);

	/* Destroying transition_mutex leaves transition_tex_mutex as it was. */
	memcpy(&tex_before, &tr->transition_tex_mutex, sizeof(tex_before));
	memcpy(&mutex_before, &tr->transition_mutex, sizeof(mutex_before));
	obs_source_transition_seam_mutex_destroy(tr);
	assert_memory_equal(&tr->transition_tex_mutex, &tex_before, sizeof(tex_before));
#ifdef __GLIBC__
	/* glibc marks a destroyed mutex: the destroy reached this one. */
	assert_memory_not_equal(&tr->transition_mutex, &mutex_before, sizeof(mutex_before));
#endif

	/* Initialized again, it locks again; the other was never touched. */
	assert_int_equal(obs_source_transition_seam_mutex_init(tr), 0);
	assert_int_equal(obs_source_transition_seam_trylock(tr), 0);
	assert_int_equal(obs_source_transition_seam_unlock(tr), 0);
	assert_memory_equal(&tr->transition_tex_mutex, &tex_before, sizeof(tex_before));

	/* And the same the other way round. */
	memcpy(&mutex_before, &tr->transition_mutex, sizeof(mutex_before));
	memcpy(&tex_before, &tr->transition_tex_mutex, sizeof(tex_before));
	obs_source_transition_seam_tex_mutex_destroy(tr);
	assert_memory_equal(&tr->transition_mutex, &mutex_before, sizeof(mutex_before));
#ifdef __GLIBC__
	assert_memory_not_equal(&tr->transition_tex_mutex, &tex_before, sizeof(tex_before));
#endif

	assert_int_equal(obs_source_transition_seam_tex_mutex_init(tr), 0);
	assert_int_equal(obs_source_transition_seam_trylock_textures(tr), 0);
	assert_int_equal(obs_source_transition_seam_unlock_textures(tr), 0);
	assert_memory_equal(&tr->transition_mutex, &mutex_before, sizeof(mutex_before));

	obs_source_transition_seam_mutex_destroy(tr);
	obs_source_transition_seam_tex_mutex_destroy(tr);
	bfree(tr);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(context_data_is_the_field),
		cmocka_unit_test(core_reads_are_the_fields),
		cmocka_unit_test(child_audio_flags_are_the_fields),
		cmocka_unit_test(child_audio_output_buf_is_the_first),
		cmocka_unit_test(init_mutex_values_sets_both),
		cmocka_unit_test(mutex_init_destroy_init),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
