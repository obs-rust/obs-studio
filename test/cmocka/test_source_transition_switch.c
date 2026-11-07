/*
 * Characterization of the transition start and set in
 * libobs/obs-source-transition.c (obs-rust/obs-studio#3794), before they move
 * to their own file and are ported: obs_transition_start() and
 * obs_transition_set(), with set_source(), activate_transition() and the
 * recursion check they use.
 *
 * test_transition_start_steps pins the rules on plain values and copies of
 * the composition on a stand-in; test_transition_recursive_destination,
 * test_transition_set_active and test_transition_set_cycle pin the recursive
 * destinations, the re-entering callbacks' clocks and the set's active
 * reference. These cases pin, through the real libobs, with a transition
 * type whose transition_start callback is counted:
 * - each start's result, slots, active flag, callback count and
 *   transition_start / transition_stop signals (on the transition and as
 *   source_transition_start / _stop on the core's handler): from idle, to
 *   slot A's source idle (refused) and while running (the running
 *   transition ends on its destination first, then starts back), to the
 *   running destination (the running one ends there and the start is
 *   refused, in manual mode too: characterized, not endorsed), and to no
 *   destination;
 * - the transition is a public source here: a private one emits no
 *   source_transition_ signal on the core's handler;
 * - the fixed duration replaces the one asked for, only while enabled;
 * - a transition_start callback that starts the transition again: the
 *   outer start's destination wins, the nested one's is let go;
 * - obs_transition_set(): both slots cleared, the new source in slot A, a
 *   stop signal only while running, and the refusal of the transition
 *   itself, with its warning;
 * - on an output channel, which children are active after each start and
 *   set.
 *
 * Transitions take core video for their render targets, so the test only
 * builds on Linux and runs under xvfb-run; without a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/platform.h>

#define CHILD_ID "test_tr_switch_child"
#define TRANSITION_ID "test_tr_switch_transition"
#define WAIT_NS 5000000000ULL

static bool obs_up;

static const char *child_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "switch child";
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

/* The sources of the running case, released by case_teardown if a failed
 * assertion ended the case first. */
static struct {
	obs_source_t *tr;
	obs_source_t *child[3];
	bool on_channel;
	/* The callback starts the transition once more, towards this. */
	obs_source_t *reenter_dest;
} live;

static unsigned start_calls;
static unsigned start_signals;
static unsigned stop_signals;
static unsigned global_start_signals;
static unsigned global_stop_signals;

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "switch transition";
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

static void tr_start(void *data)
{
	obs_source_t *tr = data;
	start_calls++;
	obs_source_t *dest = live.reenter_dest;
	if (dest) {
		live.reenter_dest = NULL;
		assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, dest));
	}
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
	.transition_start = tr_start,
	.audio_render = tr_audio_render,
};

/* The lines of the obs_transition_ functions logged since the case began,
 * one per line. */
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

static void on_start(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	assert_ptr_equal(calldata_ptr(cd, "source"), live.tr);
	start_signals++;
}

static void on_stop(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	assert_ptr_equal(calldata_ptr(cd, "source"), live.tr);
	stop_signals++;
}

/* The core's handler sees every source: only this case's transition
 * counts. */
static void on_global_start(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	if (live.tr && calldata_ptr(cd, "source") == live.tr)
		global_start_signals++;
}

static void on_global_stop(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	if (live.tr && calldata_ptr(cd, "source") == live.tr)
		global_stop_signals++;
}

static int case_setup(void **state)
{
	UNUSED_PARAMETER(state);
	logged[0] = '\0';
	start_calls = 0;
	start_signals = 0;
	stop_signals = 0;
	global_start_signals = 0;
	global_stop_signals = 0;
	return 0;
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	live.reenter_dest = NULL;
	if (live.on_channel)
		obs_set_output_source(0, NULL);
	if (live.tr) {
		signal_handler_t *sh = obs_source_get_signal_handler(live.tr);
		signal_handler_disconnect(sh, "transition_start", on_start, NULL);
		signal_handler_disconnect(sh, "transition_stop", on_stop, NULL);
		obs_transition_clear(live.tr);
	}
	obs_source_release(live.tr);
	for (size_t i = 0; i < 3; i++)
		obs_source_release(live.child[i]);
	memset(&live, 0, sizeof(live));
	return 0;
}

static void skip_without_core(void)
{
	if (!obs_up)
		skip();
}

/* A transition holding child 0 in slot A, children 1 and 2 to start
 * towards, its signals counted. */
static obs_source_t *make_transition(const char *name)
{
	for (size_t i = 0; i < 3; i++) {
		char child_name_buf[64];
		snprintf(child_name_buf, sizeof(child_name_buf), "%s %c", name, (char)('A' + i));
		live.child[i] = obs_source_create_private(CHILD_ID, child_name_buf, NULL);
		assert_non_null(live.child[i]);
	}
	live.tr = obs_source_create(TRANSITION_ID, name, NULL, NULL);
	assert_non_null(live.tr);
	signal_handler_t *sh = obs_source_get_signal_handler(live.tr);
	signal_handler_connect(sh, "transition_start", on_start, NULL);
	signal_handler_connect(sh, "transition_stop", on_stop, NULL);
	obs_transition_set(live.tr, live.child[0]);
	return live.tr;
}

/* What each slot holds, and whether the transition runs. */
static void check_slots(obs_source_t *tr, obs_source_t *a, obs_source_t *b, bool active)
{
	obs_source_t *got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_A);
	assert_ptr_equal(got, a);
	obs_source_release(got);
	got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_B);
	assert_ptr_equal(got, b);
	obs_source_release(got);
	assert_true(obs_transition_is_active(tr) == active);
}

/* The callback count, and each signal on the transition and on the core's
 * handler (always the same count). */
static void check_counts(unsigned calls, unsigned starts, unsigned stops)
{
	assert_int_equal(start_calls, calls);
	assert_int_equal(start_signals, starts);
	assert_int_equal(global_start_signals, starts);
	assert_int_equal(stop_signals, stops);
	assert_int_equal(global_stop_signals, stops);
}

/* From idle: the callback, slot B, the start signal; slot A stays. */
static void start_from_idle(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("idle");
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];
	check_counts(0, 0, 0);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, b));
	check_slots(tr, a, b, true);
	check_counts(1, 1, 0);
	assert_string_equal(logged, "");
}

/* Idle, slot A's source (or no source, with slot A empty) is refused
 * before anything is called. */
static void start_to_source_a_idle_is_refused(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("refused");
	obs_source_t *a = live.child[0];

	assert_false(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, a));
	check_slots(tr, a, NULL, false);
	assert_false(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, a));
	check_slots(tr, a, NULL, false);
	check_counts(0, 0, 0);

	obs_transition_set(tr, NULL);
	assert_false(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, NULL));
	check_slots(tr, NULL, NULL, false);
	check_counts(0, 0, 0);
	assert_string_equal(logged, "");
}

/* While running, slot A's source: the running transition is ended on its
 * destination (a stop signal), which becomes slot A, and the start goes
 * back to the old slot A. */
static void start_to_source_a_while_running(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("back");
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, b));
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, a));
	check_slots(tr, b, a, true);
	check_counts(2, 2, 1);

	/* A third destination while running: the same, from b. */
	obs_source_t *c = live.child[2];
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, c));
	check_slots(tr, a, c, true);
	check_counts(3, 3, 2);
	assert_string_equal(logged, "");
}

/* The running destination again, in either mode: the running transition
 * is ended on it first (a stop signal), it is then slot A, and the start
 * is refused. So a manual start to the destination of a running manual
 * transition is refused too, not done with nothing called (characterized,
 * not endorsed). */
static void start_to_running_destination(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("again");
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];
	obs_source_t *c = live.child[2];

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, b));
	check_slots(tr, a, b, true);
	assert_false(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, b));
	check_slots(tr, b, NULL, false);
	check_counts(1, 1, 1);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, c));
	assert_false(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, c));
	check_slots(tr, c, NULL, false);
	check_counts(2, 2, 2);

	/* From manual to auto, the same. */
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, a));
	assert_false(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, a));
	check_slots(tr, a, NULL, false);
	check_counts(3, 3, 3);
	assert_string_equal(logged, "");
}

/* No destination: the transition runs towards an empty slot B. */
static void start_to_no_destination(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("none");
	obs_source_t *a = live.child[0];

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, NULL));
	check_slots(tr, a, NULL, true);
	check_counts(1, 1, 0);
	assert_null(obs_transition_get_active_source(tr));

	/* No destination again: the running transition ends on nothing, and
	 * the start is then refused. */
	assert_false(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, NULL));
	check_slots(tr, NULL, NULL, false);
	check_counts(1, 1, 1);
	assert_string_equal(logged, "");
}

/* Wait until the transition's time passes `t`. */
static bool time_passes(obs_source_t *tr, float t)
{
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	while (obs_transition_get_time(tr) <= t && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	return obs_transition_get_time(tr) > t;
}

/* Wait until the latest video frame's time is at least `ts`
 * (os_gettime_ns()), the clock obs_transition_get_time() reads. */
static bool frame_reaches(uint64_t ts)
{
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	while (obs_get_video_frame_time() < ts && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	return obs_get_video_frame_time() >= ts;
}

/* The fixed duration replaces the one asked for while it is enabled: 200 ms
 * reaches a time of 0.5 where the 600 s asked for would still be near 0. */
static void fixed_duration_replaces_the_asked_one(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("fixed");
	obs_source_t *b = live.child[1];

	obs_transition_enable_fixed(tr, true, 200);
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 600000, b));
	assert_true(time_passes(tr, 0.5f));

	/* Disabled, the duration asked for is used: the 200 ms is still
	 * stored, and a time of 0.5 is out of reach. The time is read at the
	 * latest video frame, so wait for a frame 400 ms after the start
	 * (twice the stored duration) before reading it. */
	obs_transition_set(tr, live.child[0]);
	obs_transition_enable_fixed(tr, false, 200);
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 600000, b));
	assert_true(frame_reaches(os_gettime_ns() + 400000000ULL));
	assert_true(obs_transition_get_time(tr) < 0.01f);
	assert_true(obs_transition_is_active(tr));
}

/* The callback starts the transition again, towards c: that nested start
 * runs whole (its callback, slot B, its signal), then the outer start sets
 * its own destination over it and signals too. */
static void reentering_callback(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("reenter");
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];
	obs_source_t *c = live.child[2];

	live.reenter_dest = c;
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, b));
	check_slots(tr, a, b, true);
	check_counts(2, 2, 0);
	assert_string_equal(logged, "");
}

/* obs_transition_set(): both slots cleared, the source in slot A, the
 * transition idle, a stop signal only when it was running; the transition
 * itself is refused with a warning, and nothing changes. */
static void set_clears_both_slots(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("set");
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];
	obs_source_t *c = live.child[2];
	check_counts(0, 0, 0);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, b));
	obs_transition_set(tr, c);
	check_slots(tr, c, NULL, false);
	check_counts(1, 1, 1);

	obs_transition_set(tr, a);
	check_slots(tr, a, NULL, false);
	obs_transition_set(tr, NULL);
	check_slots(tr, NULL, NULL, false);
	check_counts(1, 1, 1);
	assert_string_equal(logged, "");

	obs_transition_set(tr, a);
	obs_transition_set(tr, tr);
	check_slots(tr, a, NULL, false);
	check_counts(1, 1, 1);
	assert_string_equal(logged, "obs_transition_set: cannot set transition 'set' to source 'set' "
				    "due to infinite recursion\n");
}

static void check_active(bool a, bool b, bool c)
{
	assert_true(obs_source_active(live.child[0]) == a);
	assert_true(obs_source_active(live.child[1]) == b);
	assert_true(obs_source_active(live.child[2]) == c);
}

/* On an output channel the transition is active, and so are the children
 * it holds: slot A's from the set, slot B's from the start; a start while
 * running lets go of the old slot A, a set lets go of both slots. */
static void active_children_follow_the_slots(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("channel");
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];
	obs_source_t *c = live.child[2];

	obs_set_output_source(0, tr);
	live.on_channel = true;
	assert_true(obs_source_active(tr));
	check_active(true, false, false);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, b));
	check_active(true, true, false);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, c));
	check_slots(tr, b, c, true);
	check_active(false, true, true);

	obs_transition_set(tr, a);
	check_active(true, false, false);

	obs_transition_set(tr, NULL);
	check_active(false, false, false);
	assert_true(obs_source_active(tr));
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
	signal_handler_connect(obs_get_signal_handler(), "source_transition_start", on_global_start, NULL);
	signal_handler_connect(obs_get_signal_handler(), "source_transition_stop", on_global_stop, NULL);
	base_get_log_handler(&prev_handler, &prev_param);
	base_set_log_handler(record_log, NULL);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up) {
		base_set_log_handler(prev_handler, prev_param);
		signal_handler_disconnect(obs_get_signal_handler(), "source_transition_start", on_global_start, NULL);
		signal_handler_disconnect(obs_get_signal_handler(), "source_transition_stop", on_global_stop, NULL);
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(start_from_idle, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(start_to_source_a_idle_is_refused, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(start_to_source_a_while_running, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(start_to_running_destination, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(start_to_no_destination, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(fixed_duration_replaces_the_asked_one, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(reentering_callback, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(set_clears_both_slots, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(active_children_follow_the_slots, case_setup, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
