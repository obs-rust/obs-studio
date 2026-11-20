/* C-source characterization of the private clear-mix seam. The recording
 * teardown boundary checks ordering and frees empty mixes; it does not model
 * GPU/video teardown. No OBS startup, graphics device or video thread.
 * External mode is reserved for the later swap's unchanged assertions. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <errno.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

static struct obs_core core;
static struct obs_core *current_core;
static struct obs_canvas canvas;
static struct obs_weak_canvas control;
static struct obs_source source;
static struct obs_weak_source source_control;
static struct obs_core_video_mix *expected_mix;
static size_t expected_count;
static unsigned locks, unlocks, frees;
static bool locked, retain_mix;

static int record_lock(pthread_mutex_t *mutex)
{
	assert_ptr_equal(mutex, &core.video.mixes_mutex);
	assert_false(locked);
	assert_ptr_equal(canvas.mix, expected_mix);
	int result = pthread_mutex_lock(mutex);
	assert_int_equal(result, 0);
	locked = true;
	locks++;
	return result;
}

static int record_unlock(pthread_mutex_t *mutex)
{
	assert_true(locked);
	assert_ptr_equal(mutex, &core.video.mixes_mutex);
	/* The owner pointer is cleared only after the list lock is released. */
	assert_ptr_equal(canvas.mix, expected_mix);
	int result = pthread_mutex_unlock(mutex);
	assert_int_equal(result, 0);
	locked = false;
	unlocks++;
	return result;
}

static void record_free(struct obs_core_video_mix *mix)
{
	assert_ptr_equal(mix, expected_mix);
	assert_true(locked);
	assert_int_equal(pthread_mutex_trylock(&core.video.mixes_mutex), EBUSY);
	assert_int_equal(core.video.mixes.num, expected_count);
	assert_ptr_equal(canvas.mix, mix);
	assert_null(mix->video);
	frees++;
	if (!retain_mix)
		bfree(mix);
}

#ifndef CANVAS_CLEAR_MIX_EXTERNAL
#define obs current_core
#define obs_free_video_mix record_free
#pragma push_macro("pthread_mutex_lock")
#pragma push_macro("pthread_mutex_unlock")
#undef pthread_mutex_lock
#undef pthread_mutex_unlock
#define pthread_mutex_lock record_lock
#define pthread_mutex_unlock record_unlock
#include "libobs/obs-canvas-clear-mix.c.h"
#pragma pop_macro("pthread_mutex_unlock")
#pragma pop_macro("pthread_mutex_lock")
#undef obs_free_video_mix
#undef obs
#endif

static int setup(void **unused)
{
	(void)unused;
	memset(&core, 0, sizeof(core));
	memset(&canvas, 0, sizeof(canvas));
	memset(&control, 0, sizeof(control));
	memset(&source, 0, sizeof(source));
	memset(&source_control, 0, sizeof(source_control));
	control.canvas = &canvas;
	control.ref.refs = 17;
	control.ref.weak_refs = 23;
	canvas.context.control = (struct obs_weak_object *)&control;
	source_control.source = &source;
	source_control.ref.refs = 31;
	source_control.ref.weak_refs = 47;
	source.context.control = (struct obs_weak_object *)&source_control;
	canvas.sources = &source;
	canvas.view.channels[0] = &source;
	canvas.flags = UINT32_MAX;
	current_core = &core;
	expected_mix = NULL;
	expected_count = 0;
	locks = unlocks = frees = 0;
	locked = retain_mix = false;
	assert_int_equal(pthread_mutex_init(&core.video.mixes_mutex, NULL), 0);
	return 0;
}

static void check_clear(void)
{
	struct obs_canvas before = canvas;
	struct obs_weak_canvas refs_before = control;
	struct obs_weak_source source_refs_before = source_control;
	struct obs_source source_before = source;
	obs_canvas_clear_mix(&canvas);
	before.mix = NULL;
	assert_memory_equal(&canvas, &before, sizeof(canvas));
	assert_memory_equal(&control, &refs_before, sizeof(control));
	assert_memory_equal(&source_control, &source_refs_before, sizeof(source_control));
	assert_memory_equal(&source, &source_before, sizeof(source));
	assert_false(locked);
	assert_int_equal(pthread_mutex_trylock(&core.video.mixes_mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&core.video.mixes_mutex), 0);
}

static int teardown(void **unused)
{
	(void)unused;
	da_free(core.video.mixes);
	assert_int_equal(pthread_mutex_destroy(&core.video.mixes_mutex), 0);
	return 0;
}

static void no_mix(void **unused)
{
	(void)unused;
	/* The early return does not even read the core or acquire its mutex. */
	current_core = NULL;
	check_clear();
	current_core = &core;
	assert_int_equal(pthread_mutex_lock(&core.video.mixes_mutex), 0);
	obs_canvas_clear_mix(&canvas);
	assert_int_equal(pthread_mutex_unlock(&core.video.mixes_mutex), 0);
	assert_int_equal(locks + unlocks + frees, 0);
}

static void list_case(size_t count, size_t position)
{
	struct obs_core_video_mix *mixes[4];
	for (size_t i = 0; i < count; i++) {
		mixes[i] = bzalloc(sizeof(*mixes[i]));
		assert_non_null(mixes[i]);
		da_push_back(core.video.mixes, &mixes[i]);
	}
	canvas.mix = expected_mix = mixes[position];
	expected_count = count - 1;
	long allocations = bnum_allocs();
	check_clear();
	assert_int_equal(bnum_allocs(), allocations - 1);
	assert_int_equal(locks, 1);
	assert_int_equal(unlocks, 1);
	assert_int_equal(frees, 1);
	assert_int_equal(core.video.mixes.num, count - 1);
	size_t index = 0;
	for (size_t i = 0; i < count; i++) {
		if (i == position)
			continue;
		assert_ptr_equal(core.video.mixes.array[index++], mixes[i]);
		bfree(mixes[i]);
	}
	/* A second clear neither locks nor frees again. */
	check_clear();
	assert_int_equal(locks, 1);
	assert_int_equal(frees, 1);
	da_free(core.video.mixes);
}

static void one_mix(void **unused)
{
	(void)unused;
	list_case(1, 0);
}

static void several_mixes(void **unused)
{
	(void)unused;
	for (size_t position = 0; position < 4; position++) {
		locks = unlocks = frees = 0;
		list_case(4, position);
	}
}

static void unregistered_mix(void **unused)
{
	(void)unused;
	struct obs_core_video_mix *other = bzalloc(sizeof(*other));
	struct obs_core_video_mix *orphan = bzalloc(sizeof(*orphan));
	da_push_back(core.video.mixes, &other);
	canvas.mix = expected_mix = orphan;
	long allocations = bnum_allocs();
	/* Characterized, not endorsed: an unregistered mix is detached, not freed. */
	check_clear();
	assert_int_equal(bnum_allocs(), allocations);
	assert_int_equal(core.video.mixes.num, 1);
	assert_ptr_equal(core.video.mixes.array[0], other);
	assert_int_equal(locks, 1);
	assert_int_equal(unlocks, 1);
	assert_int_equal(frees, 0);
	bfree(orphan);
	bfree(other);
}

static void first_match_only(void **unused)
{
	(void)unused;
	struct obs_core_video_mix *mix = bzalloc(sizeof(*mix));
	da_push_back(core.video.mixes, &mix);
	da_push_back(core.video.mixes, &mix);
	canvas.mix = expected_mix = mix;
	expected_count = 1;
	/* Keep the allocation alive to inspect the duplicate pointer safely.
	 * Characterized, not endorsed: the production list must not contain duplicates. */
	retain_mix = true;
	check_clear();
	assert_int_equal(frees, 1);
	assert_int_equal(core.video.mixes.num, 1);
	assert_ptr_equal(core.video.mixes.array[0], mix);
	bfree(mix);
}

#ifdef CANVAS_CLEAR_MIX_NULL_PROBE
extern void canvas_clear_mix_null_probe(void (*clear)(obs_canvas_t *));

static void null_canvas(void **unused)
{
	(void)unused;
	canvas_clear_mix_null_probe(obs_canvas_clear_mix);
}
#endif

int main(void)
{
	const struct CMUnitTest tests[] = {
#ifdef CANVAS_CLEAR_MIX_NULL_PROBE
		cmocka_unit_test(null_canvas),
#endif
		cmocka_unit_test_setup_teardown(no_mix, setup, teardown),
		cmocka_unit_test_setup_teardown(one_mix, setup, teardown),
		cmocka_unit_test_setup_teardown(several_mixes, setup, teardown),
		cmocka_unit_test_setup_teardown(unregistered_mix, setup, teardown),
		cmocka_unit_test_setup_teardown(first_match_only, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
