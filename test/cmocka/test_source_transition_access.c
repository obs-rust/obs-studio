/*
 * Characterization of the transition source getters and manual controls in
 * libobs/obs-source-transition.c (obs-rust/obs-studio#3582), before they move
 * to their own file and are ported: obs_transition_get_source(),
 * obs_transition_get_active_source(), obs_transition_set_manual_torque() and
 * obs_transition_set_manual_time().
 *
 * Other transition tests call the getters along the way, and
 * test_source_transition_slots and test_source_transition_size see the
 * manual value move. These cases pin, through the real libobs:
 * - each getter returns a new reference on what its slot holds: slot A or
 *   B, and the active source, which is B while the transition runs (auto or
 *   manual) and A otherwise; NULL for an empty slot, for NULL (logged at
 *   debug level, by get_active_source under "obs_transition_get_source":
 *   characterized, not endorsed) and, without a log, for a source that is
 *   not a transition;
 * - the torque and clamp the next tick moves the manual value with, each
 *   setter call replacing both;
 * - obs_transition_set_manual_time() sets the target, and, in manual mode
 *   with a time of 0 (or -0), ends the transition on slot A; not with any
 *   other time, NaN included, and not in auto mode.
 * The setters are called on transitions only: neither checks its argument
 * (characterized, not endorsed).
 *
 * The tick runs on the graphics thread, once per frame, so the manual cases
 * wait for the frame that moves the value, with a 5 s bound that only turns
 * a hang into a failure. Transitions take core video for their render
 * targets, so the test only builds on Linux and runs under xvfb-run; without
 * a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/platform.h>

#define CHILD_ID "test_tr_access_child"
#define TRANSITION_ID "test_tr_access_transition"
#define WAIT_NS 5000000000ULL

static bool obs_up;

static const char *child_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "access child";
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

static void child_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(effect);
}

static struct obs_source_info child_info = {
	.id = CHILD_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
	.get_name = child_name,
	.create = child_create,
	.destroy = child_destroy,
	.get_width = child_size,
	.get_height = child_size,
	.video_render = child_render,
};

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "access transition";
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

/* The lines of the obs_transition_ functions logged since the case began,
 * one per line. Other lines (a source destroyed on the destruction thread,
 * at any time) are left out. */
static char logged[8192];
static log_handler_t prev_handler;
static void *prev_param;

static void record_log(int lvl, const char *msg, va_list args, void *p)
{
	UNUSED_PARAMETER(p);
	char line[512];
	va_list copy;
	va_copy(copy, args);
	vsnprintf(line, sizeof(line), msg, copy);
	va_end(copy);
	if (strncmp(line, "obs_transition_", strlen("obs_transition_")) == 0) {
		size_t len = strlen(logged);
		snprintf(logged + len, sizeof(logged) - len, "%s\n", line);
	}
	if (prev_handler)
		prev_handler(lvl, msg, args, prev_param);
}

/* The sources of the running case, released by case_teardown if a failed
 * assertion ended the case first. */
static struct {
	obs_source_t *tr;
	obs_source_t *child[2];
	obs_weak_source_t *weak[2];
	obs_source_t *got;
	obs_source_t *input;
} live;

static int case_setup(void **state)
{
	UNUSED_PARAMETER(state);
	logged[0] = '\0';
	return 0;
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (live.tr)
		obs_transition_clear(live.tr);
	obs_source_release(live.tr);
	obs_source_release(live.got);
	obs_source_release(live.input);
	for (size_t i = 0; i < 2; i++) {
		obs_source_release(live.child[i]);
		obs_weak_source_release(live.weak[i]);
	}
	memset(&live, 0, sizeof(live));
	return 0;
}

static void skip_without_core(void)
{
	if (!obs_up)
		skip();
}

/* A transition holding child 0 in slot A, and child 1 to start towards. */
static obs_source_t *make_transition(const char *name)
{
	char child_names[2][64];
	snprintf(child_names[0], sizeof(child_names[0]), "%s A", name);
	snprintf(child_names[1], sizeof(child_names[1]), "%s B", name);
	for (size_t i = 0; i < 2; i++) {
		live.child[i] = obs_source_create_private(CHILD_ID, child_names[i], NULL);
		assert_non_null(live.child[i]);
		live.weak[i] = obs_source_get_weak_source(live.child[i]);
	}
	live.tr = obs_source_create_private(TRANSITION_ID, name, NULL);
	assert_non_null(live.tr);
	obs_transition_set(live.tr, live.child[0]);
	return live.tr;
}

static bool alive(size_t i)
{
	obs_source_t *s = obs_weak_source_get_source(live.weak[i]);
	obs_source_release(s);
	return s != NULL;
}

/* Wait until child i has no reference left. The graphics thread holds a
 * reference on every source while it ticks them, so the last one can go
 * a moment after the test's release. */
static bool gone(size_t i)
{
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	while (alive(i) && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	return !alive(i);
}

/* Drop the test's own references on both children: from then on only the
 * transition, and what a getter returned, hold them. */
static void drop_children(void)
{
	for (size_t i = 0; i < 2; i++) {
		obs_source_release(live.child[i]);
		live.child[i] = NULL;
	}
}

/* What a getter returned is the child's own pointer, and a reference of its
 * own: after the transition lets go, the child lives until it is
 * released. */
static void check_returned_reference(obs_source_t *got, size_t i)
{
	live.got = got;
	obs_source_t *s = obs_weak_source_get_source(live.weak[i]);
	assert_ptr_equal(got, s);
	obs_source_release(s);

	obs_transition_clear(live.tr);
	assert_true(alive(i));
	obs_source_release(live.got);
	live.got = NULL;
	assert_true(gone(i));
}

/* obs_transition_get_source(): slot A, slot B, an empty slot. */
static void get_source_returns_each_slot(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("get");
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];

	/* Idle: slot B is empty. */
	obs_source_t *got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_B);
	assert_null(got);
	got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_A);
	assert_ptr_equal(got, a);
	obs_source_release(got);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, b));
	got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_B);
	assert_ptr_equal(got, b);
	obs_source_release(got);

	drop_children();
	check_returned_reference(obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_A), 0);
}

static void get_source_b_holds_a_reference(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("get B");
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, live.child[1]));
	drop_children();
	check_returned_reference(obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_B), 1);
}

/* obs_transition_get_active_source(): A while idle, B while the
 * transition runs, in auto or manual mode, and A again once it ends. */
static void active_source_follows_the_transition(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("active");
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];

	obs_source_t *got = obs_transition_get_active_source(tr);
	assert_ptr_equal(got, a);
	obs_source_release(got);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, b));
	got = obs_transition_get_active_source(tr);
	assert_ptr_equal(got, b);
	obs_source_release(got);

	/* obs_transition_set() ends it on the source it is given. */
	obs_transition_set(tr, b);
	got = obs_transition_get_active_source(tr);
	assert_ptr_equal(got, b);
	obs_source_release(got);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, a));
	got = obs_transition_get_active_source(tr);
	assert_ptr_equal(got, a);
	obs_source_release(got);

	drop_children();
	check_returned_reference(obs_transition_get_active_source(tr), 0);
}

/* NULL: no source, and a debug line; get_active_source logs under
 * get_source's name (characterized, not endorsed). A source that is not a
 * transition: no source, no line. */
static void getters_refuse_what_is_not_a_transition(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	assert_null(obs_transition_get_source(NULL, OBS_TRANSITION_SOURCE_A));
	assert_string_equal(logged, "obs_transition_get_source: Null 'transition' parameter\n");
	logged[0] = '\0';
	assert_null(obs_transition_get_active_source(NULL));
	assert_string_equal(logged, "obs_transition_get_source: Null 'transition' parameter\n");

	live.input = obs_source_create_private(CHILD_ID, "access input", NULL);
	assert_non_null(live.input);
	logged[0] = '\0';
	assert_null(obs_transition_get_source(live.input, OBS_TRANSITION_SOURCE_A));
	assert_null(obs_transition_get_source(live.input, OBS_TRANSITION_SOURCE_B));
	assert_null(obs_transition_get_active_source(live.input));
	assert_string_equal(logged, "");
}

/* Wait for the tick that moves the manual value away from `from`. */
static float wait_for_change(obs_source_t *tr, float from)
{
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	float t = obs_transition_get_time(tr);
	while (t == from && os_gettime_ns() < deadline) {
		os_sleep_ms(5);
		t = obs_transition_get_time(tr);
	}
	if (t == from) {
		fprintf(stderr, "the manual value stayed at %g\n", from);
		fail();
	}
	return t;
}

/* The torque and the clamp the tick uses are the last ones set: a torque
 * of 0 snaps, a small torque with no clamp only starts on its way, and a
 * clamp larger than the step reaches the target; each call replaces both. */
static void manual_torque_sets_torque_and_clamp(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("torque");
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, live.child[1]));

	obs_transition_set_manual_torque(tr, 0.0f, 0.0f);
	obs_transition_set_manual_time(tr, 0.25f);
	assert_true(wait_for_change(tr, 0.0f) == 0.25f);

	obs_transition_set_manual_torque(tr, 0.1f, 1000.0f);
	obs_transition_set_manual_time(tr, 0.75f);
	assert_true(wait_for_change(tr, 0.25f) == 0.75f);

	/* The same torque, the clamp replaced by 0. */
	obs_transition_set_manual_torque(tr, 0.1f, 0.0f);
	obs_transition_set_manual_time(tr, 0.25f);
	float t = wait_for_change(tr, 0.75f);
	assert_true(t > 0.25f && t < 0.75f);

	/* The torque replaced by 0 again: a snap from where it is. */
	obs_transition_set_manual_torque(tr, 0.0f, 0.0f);
	obs_transition_set_manual_time(tr, 0.5f);
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	while (obs_transition_get_time(tr) != 0.5f && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	assert_true(obs_transition_get_time(tr) == 0.5f);
	assert_true(obs_transition_is_active(tr));
}

/* Whether a manual time of `t` ends a transition started in `mode` on slot
 * A. */
static void check_manual_time(const char *name, enum obs_transition_mode mode, float t, bool ends)
{
	obs_source_t *tr = make_transition(name);
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];
	assert_true(obs_transition_start(tr, mode, 60000, b));
	assert_true(obs_transition_is_active(tr));
	logged[0] = '\0';

	obs_transition_set_manual_time(tr, t);

	if (ends) {
		assert_false(obs_transition_is_active(tr));
		obs_source_t *got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_A);
		assert_ptr_equal(got, a);
		obs_source_release(got);
		assert_null(obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_B));
		got = obs_transition_get_active_source(tr);
		assert_ptr_equal(got, a);
		obs_source_release(got);
		assert_true(obs_transition_get_time(tr) == 0.0f);
	} else {
		assert_true(obs_transition_is_active(tr));
		obs_source_t *got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_B);
		assert_ptr_equal(got, b);
		obs_source_release(got);
	}
	assert_string_equal(logged, "");
	case_teardown(NULL);
}

/* The reset to slot A: manual mode and a time of 0 or -0 only. */
static void manual_time_resets_to_a_at_zero(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	check_manual_time("manual 0", OBS_TRANSITION_MODE_MANUAL, 0.0f, true);
	check_manual_time("manual -0", OBS_TRANSITION_MODE_MANUAL, -0.0f, true);
	check_manual_time("manual 0.5", OBS_TRANSITION_MODE_MANUAL, 0.5f, false);
	check_manual_time("manual 1", OBS_TRANSITION_MODE_MANUAL, 1.0f, false);
	check_manual_time("manual NaN", OBS_TRANSITION_MODE_MANUAL, NAN, false);
	check_manual_time("auto 0", OBS_TRANSITION_MODE_AUTO, 0.0f, false);
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
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(record_log, NULL);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up) {
		base_set_log_handler(prev_handler, prev_param);
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(get_source_returns_each_slot, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(get_source_b_holds_a_reference, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(active_source_follows_the_transition, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(getters_refuse_what_is_not_a_transition, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(manual_torque_sets_torque_and_clamp, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(manual_time_resets_to_a_at_zero, case_setup, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
