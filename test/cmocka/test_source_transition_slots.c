/*
 * Characterization of the child slots of a transition in
 * libobs/obs-source-transition.c (obs-rust/obs-studio#3530), before they move
 * to their own file and are ported: obs_transition_clear(),
 * obs_transition_enum_sources(), and the swap of obs_transition_swap_begin(),
 * copy_source_state() and obs_transition_swap_end().
 *
 * test_transition_swap, test_transition_swap_active and
 * test_transition_swap_cycle pin the swap's handover of a running
 * transition and its refusal of a source that contains the destination.
 * These cases pin what the slots hold and who holds a reference: clear
 * empties both slots and drops both references, enum_sources visits slot A
 * and then slot B and skips an empty one, and the swap hands both slots and
 * the running state from the old transition to the new one, drops what the
 * new one held, and leaves the old one empty once it ends. A source's last
 * reference is seen through a weak reference, which stops giving the source
 * the moment its last reference goes. Odd behaviour is characterized, not
 * endorsed.
 *
 * The swap is called as the frontend calls it: begin, then end, on the same
 * pair, from one thread. Nothing renders the transitions, so their state
 * stays where the calls leave it. Transitions take core video for their
 * render targets, so the test only builds on Linux and runs under xvfb-run;
 * without a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>

#define CHILD_ID "test_tr_slots_child"
#define TRANSITION_ID "test_tr_slots_transition"

static bool obs_up;

static const char *child_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "slots child";
}

static void *child_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void child_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static uint32_t child_size(void *data)
{
	UNUSED_PARAMETER(data);
	return 16;
}

static struct obs_source_info child_info = {
	.id = CHILD_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = child_name,
	.create = child_create,
	.destroy = child_destroy,
	.get_width = child_size,
	.get_height = child_size,
};

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "slots transition";
}

static void *tr_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void tr_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static float mix_a(void *data, float t)
{
	UNUSED_PARAMETER(data);
	return 1.0f - t;
}

static float mix_b(void *data, float t)
{
	UNUSED_PARAMETER(data);
	return t;
}

static bool tr_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
			    size_t channels, size_t sample_rate)
{
	return obs_transition_audio_render(data, ts_out, audio, mixers, channels, sample_rate, mix_a, mix_b);
}

static struct obs_source_info transition_info = {
	.id = TRANSITION_ID,
	.type = OBS_SOURCE_TYPE_TRANSITION,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = tr_name,
	.create = tr_create,
	.destroy = tr_destroy,
	.audio_render = tr_audio_render,
};

#define MAX_CHILDREN 3

/* The sources of the running case, released by case_teardown if a failed
 * assertion ended the case first. */
static struct {
	obs_source_t *tr[2];
	obs_source_t *child[MAX_CHILDREN];
	obs_weak_source_t *weak[MAX_CHILDREN];
	obs_source_t *input;
} live;

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	for (size_t i = 0; i < 2; i++) {
		if (live.tr[i])
			obs_transition_clear(live.tr[i]);
		obs_source_release(live.tr[i]);
	}
	for (size_t i = 0; i < MAX_CHILDREN; i++) {
		obs_source_release(live.child[i]);
		obs_weak_source_release(live.weak[i]);
	}
	obs_source_release(live.input);
	memset(&live, 0, sizeof(live));
	return 0;
}

static void skip_without_core(void)
{
	if (!obs_up)
		skip();
}

static obs_source_t *make_transition(size_t i, const char *name)
{
	live.tr[i] = obs_source_create_private(TRANSITION_ID, name, NULL);
	assert_non_null(live.tr[i]);
	assert_int_equal(obs_source_get_type(live.tr[i]), OBS_SOURCE_TYPE_TRANSITION);
	return live.tr[i];
}

static obs_source_t *make_child(size_t i, const char *name)
{
	live.child[i] = obs_source_create_private(CHILD_ID, name, NULL);
	assert_non_null(live.child[i]);
	live.weak[i] = obs_source_get_weak_source(live.child[i]);
	return live.child[i];
}

/* Drop the test's own reference on child i: from then on only the
 * transitions hold it. */
static void drop_child(size_t i)
{
	obs_source_release(live.child[i]);
	live.child[i] = NULL;
}

/* Whether anything still holds a reference on child i. */
static bool child_alive(size_t i)
{
	obs_source_t *s = obs_weak_source_get_source(live.weak[i]);
	obs_source_release(s);
	return s != NULL;
}

/* What slot `target` of `tr` holds, without keeping a reference. */
static obs_source_t *slot(obs_source_t *tr, enum obs_transition_target target)
{
	obs_source_t *s = obs_transition_get_source(tr, target);
	obs_source_release(s);
	return s;
}

/* Child i itself, while something holds it, without keeping a reference. */
static obs_source_t *child(size_t i)
{
	obs_source_t *s = obs_weak_source_get_source(live.weak[i]);
	obs_source_release(s);
	assert_non_null(s);
	return s;
}

/* A transition holding child 0 in slot A and child 1 in slot B, running
 * towards child 1. */
static obs_source_t *make_running(size_t i, const char *name)
{
	obs_source_t *tr = make_transition(i, name);
	obs_transition_set(tr, live.child[0]);
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, live.child[1]));
	assert_true(obs_transition_is_active(tr));
	return tr;
}

static void clear_empties_both_slots(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	make_child(0, "clear A");
	make_child(1, "clear B");
	obs_source_t *tr = make_running(0, "clear");
	drop_child(0);
	drop_child(1);

	assert_ptr_equal(slot(tr, OBS_TRANSITION_SOURCE_A), child(0));
	assert_ptr_equal(slot(tr, OBS_TRANSITION_SOURCE_B), child(1));

	obs_transition_clear(tr);
	assert_null(slot(tr, OBS_TRANSITION_SOURCE_A));
	assert_null(slot(tr, OBS_TRANSITION_SOURCE_B));
	assert_false(obs_transition_is_active(tr));
	/* Both references dropped. */
	assert_false(child_alive(0));
	assert_false(child_alive(1));

	/* Clearing an empty transition, NULL or a source that is not a
	 * transition does nothing. */
	obs_transition_clear(tr);
	assert_null(slot(tr, OBS_TRANSITION_SOURCE_A));
	obs_transition_clear(NULL);
	live.input = obs_source_create_private(CHILD_ID, "clear input", NULL);
	obs_transition_clear(live.input);
}

/* The order obs_source_enum_active_sources() reports a transition's
 * children in, which is obs_transition_enum_sources()'s. */
#define MAX_SEEN 4
static struct {
	obs_source_t *parent[MAX_SEEN];
	obs_source_t *child[MAX_SEEN];
	size_t n;
} seen;

static void record_child(obs_source_t *parent, obs_source_t *child_source, void *param)
{
	assert_ptr_equal(param, &seen);
	assert_true(seen.n < MAX_SEEN);
	seen.parent[seen.n] = parent;
	seen.child[seen.n] = child_source;
	seen.n++;
}

static void enumerate(obs_source_t *tr)
{
	memset(&seen, 0, sizeof(seen));
	obs_source_enum_active_sources(tr, record_child, &seen);
}

static void enum_visits_a_then_b(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	make_child(0, "enum A");
	make_child(1, "enum B");
	obs_source_t *tr = make_transition(0, "enum");

	enumerate(tr);
	assert_int_equal(seen.n, 0);

	obs_transition_set(tr, live.child[0]);
	enumerate(tr);
	assert_int_equal(seen.n, 1);
	assert_ptr_equal(seen.parent[0], tr);
	assert_ptr_equal(seen.child[0], live.child[0]);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, live.child[1]));
	enumerate(tr);
	assert_int_equal(seen.n, 2);
	assert_ptr_equal(seen.parent[0], tr);
	assert_ptr_equal(seen.child[0], live.child[0]);
	assert_ptr_equal(seen.parent[1], tr);
	assert_ptr_equal(seen.child[1], live.child[1]);

	/* Slot A empty, slot B filled: only B. */
	obs_transition_set(tr, NULL);
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, live.child[1]));
	assert_null(slot(tr, OBS_TRANSITION_SOURCE_A));
	enumerate(tr);
	assert_int_equal(seen.n, 1);
	assert_ptr_equal(seen.child[0], live.child[1]);

	obs_transition_clear(tr);
	enumerate(tr);
	assert_int_equal(seen.n, 0);
}

/* The swap hands both slots and the running state over. Between begin and
 * end both transitions hold both children; the end clears the old one, so
 * only the new one holds them. */
static void swap_hands_over_both_slots(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	make_child(0, "swap A");
	make_child(1, "swap B");
	obs_source_t *old_tr = make_running(0, "swap old");
	obs_source_t *new_tr = make_transition(1, "swap new");
	drop_child(0);
	drop_child(1);

	obs_transition_swap_begin(new_tr, old_tr);
	assert_ptr_equal(slot(new_tr, OBS_TRANSITION_SOURCE_A), child(0));
	assert_ptr_equal(slot(new_tr, OBS_TRANSITION_SOURCE_B), child(1));
	assert_true(obs_transition_is_active(new_tr));
	assert_ptr_equal(slot(old_tr, OBS_TRANSITION_SOURCE_A), child(0));
	assert_ptr_equal(slot(old_tr, OBS_TRANSITION_SOURCE_B), child(1));
	assert_true(obs_transition_is_active(old_tr));
	obs_transition_swap_end(new_tr, old_tr);

	assert_null(slot(old_tr, OBS_TRANSITION_SOURCE_A));
	assert_null(slot(old_tr, OBS_TRANSITION_SOURCE_B));
	assert_false(obs_transition_is_active(old_tr));
	assert_ptr_equal(slot(new_tr, OBS_TRANSITION_SOURCE_A), child(0));
	assert_ptr_equal(slot(new_tr, OBS_TRANSITION_SOURCE_B), child(1));
	assert_true(obs_transition_is_active(new_tr));

	/* The new transition holds the only references. */
	obs_transition_clear(new_tr);
	assert_false(child_alive(0));
	assert_false(child_alive(1));
}

/* What the new transition held before the swap is dropped; an empty slot
 * of the old one empties the new one's. */
static void swap_drops_what_the_new_one_held(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	make_child(0, "drop A");
	make_child(1, "drop B");
	make_child(2, "drop C");
	obs_source_t *old_tr = make_transition(0, "drop old");
	obs_source_t *new_tr = make_transition(1, "drop new");
	obs_transition_set(old_tr, live.child[0]);
	obs_transition_set(new_tr, live.child[2]);
	assert_true(obs_transition_start(new_tr, OBS_TRANSITION_MODE_AUTO, 60000, live.child[1]));
	drop_child(0);
	drop_child(1);
	drop_child(2);

	obs_transition_swap_begin(new_tr, old_tr);
	obs_transition_swap_end(new_tr, old_tr);

	assert_ptr_equal(slot(new_tr, OBS_TRANSITION_SOURCE_A), child(0));
	assert_null(slot(new_tr, OBS_TRANSITION_SOURCE_B));
	/* The old one was not transitioning, so neither is the new one now. */
	assert_false(obs_transition_is_active(new_tr));
	assert_false(child_alive(1));
	assert_false(child_alive(2));
	assert_null(slot(old_tr, OBS_TRANSITION_SOURCE_A));
}

/* A swap of a transition with itself does nothing, and returns. */
static void swap_with_itself_does_nothing(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	make_child(0, "self A");
	make_child(1, "self B");
	obs_source_t *tr = make_running(0, "self");

	obs_transition_swap_begin(tr, tr);
	obs_transition_swap_end(tr, tr);

	assert_ptr_equal(slot(tr, OBS_TRANSITION_SOURCE_A), live.child[0]);
	assert_ptr_equal(slot(tr, OBS_TRANSITION_SOURCE_B), live.child[1]);
	assert_true(obs_transition_is_active(tr));
}

/* The new transition in a slot of the old one is not stored in the new
 * one: it would hold itself. */
static void swap_does_not_store_the_new_transition(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *old_tr = make_transition(0, "nested old");
	obs_source_t *new_tr = make_transition(1, "nested new");
	obs_transition_set(old_tr, new_tr);
	assert_ptr_equal(slot(old_tr, OBS_TRANSITION_SOURCE_A), new_tr);

	obs_transition_swap_begin(new_tr, old_tr);
	obs_transition_swap_end(new_tr, old_tr);

	assert_null(slot(new_tr, OBS_TRANSITION_SOURCE_A));
	assert_null(slot(new_tr, OBS_TRANSITION_SOURCE_B));
	assert_null(slot(old_tr, OBS_TRANSITION_SOURCE_A));
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	obs_register_source(&child_info);
	obs_register_source(&transition_info);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(clear_empties_both_slots, case_teardown),
		cmocka_unit_test_teardown(enum_visits_a_then_b, case_teardown),
		cmocka_unit_test_teardown(swap_hands_over_both_slots, case_teardown),
		cmocka_unit_test_teardown(swap_drops_what_the_new_one_held, case_teardown),
		cmocka_unit_test_teardown(swap_with_itself_does_nothing, case_teardown),
		cmocka_unit_test_teardown(swap_does_not_store_the_new_transition, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
