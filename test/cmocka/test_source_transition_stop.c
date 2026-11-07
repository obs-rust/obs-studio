/*
 * Characterization of the transition clock read and stop helpers in
 * libobs/obs-source-transition.c (obs-rust/obs-studio#3836), before they move
 * to their own file and are ported: obs_transition_get_time() and
 * obs_transition_force_stop(), with calc_time(), get_video_time(),
 * obs_transition_stop() and handle_stop() they use.
 *
 * The source_transition_clock parity tests pin the time rule on plain
 * values, test_source_transition_settings pins that a force stop leaves the
 * transition running, and the render-step tests pin when a render stops it.
 * These cases pin, through the real libobs, with a transition type whose
 * transition_stop callback is counted:
 * - obs_transition_get_time() in auto mode: 1.0 on a transition never
 *   started (no duration), between 0 and 0.01 early in a long start, 1.0
 *   once the latest video frame is past a short start's end, and still 1.0
 *   while it is left running (nothing renders it);
 * - in manual mode: 0.0 after the start, then the manual time each tick
 *   snaps to, and 0.0 again once obs_transition_set() ends it (the mode stays
 *   manual, the value is reset);
 * - obs_transition_force_stop(): the callback and the transition_stop
 *   signal, on the transition and as source_transition_stop on the core's
 *   handler, idle or running, the slots and the running state unchanged;
 *   without a callback only the signals; a private transition emits nothing
 *   on the core's handler;
 * - the stop at the end of an auto transition that nothing shows
 *   (obs_transition_video_render_direct() past its end): slot B moves to
 *   slot A and slot B empties, the video stop and stop signals and the
 *   callback, once;
 * - the stop of a transition on an output channel, once its video and audio
 *   have both ended: the same slots, and the old slot A is no longer active.
 *
 * Only valid transitions: these functions do not check for NULL.
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
#include <util/threading.h>

#define CHILD_ID "test_tr_stop_child"
#define TRANSITION_ID "test_tr_stop_transition"
#define BARE_TRANSITION_ID "test_tr_stop_bare_transition"
#define WAIT_NS 5000000000ULL

static bool obs_up;

static const char *child_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "stop child";
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
} live;

static volatile long stop_calls;
static volatile long stop_signals;
static volatile long video_stop_signals;
static volatile long global_stop_signals;

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "stop transition";
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

static void tr_stop(void *data)
{
	assert_ptr_equal(data, live.tr);
	os_atomic_inc_long(&stop_calls);
}

static void tr_video_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(effect);
	obs_transition_video_render(data, NULL);
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
	.transition_stop = tr_stop,
	.video_render = tr_video_render,
	.audio_render = tr_audio_render,
};

/* The same without a transition_stop callback. */
static struct obs_source_info bare_transition_info = {
	.id = BARE_TRANSITION_ID,
	.type = OBS_SOURCE_TYPE_TRANSITION,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = tr_name,
	.create = tr_create,
	.destroy = tr_destroy,
	.video_render = tr_video_render,
	.audio_render = tr_audio_render,
};

static void on_stop(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	assert_ptr_equal(calldata_ptr(cd, "source"), live.tr);
	os_atomic_inc_long(&stop_signals);
}

static void on_video_stop(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	assert_ptr_equal(calldata_ptr(cd, "source"), live.tr);
	os_atomic_inc_long(&video_stop_signals);
}

/* The core's handler sees every source: only this case's transition
 * counts. */
static void on_global_stop(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	if (live.tr && calldata_ptr(cd, "source") == live.tr)
		os_atomic_inc_long(&global_stop_signals);
}

static int case_setup(void **state)
{
	UNUSED_PARAMETER(state);
	os_atomic_set_long(&stop_calls, 0);
	os_atomic_set_long(&stop_signals, 0);
	os_atomic_set_long(&video_stop_signals, 0);
	os_atomic_set_long(&global_stop_signals, 0);
	return 0;
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (live.on_channel)
		obs_set_output_source(0, NULL);
	if (live.tr) {
		signal_handler_t *sh = obs_source_get_signal_handler(live.tr);
		signal_handler_disconnect(sh, "transition_stop", on_stop, NULL);
		signal_handler_disconnect(sh, "transition_video_stop", on_video_stop, NULL);
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

/* A transition of type `id` holding child 0 in slot A, children 1 and 2 to
 * start towards, its stop signals counted. */
static obs_source_t *make_transition(const char *id, const char *name, bool private_source)
{
	for (size_t i = 0; i < 3; i++) {
		char child_name_buf[64];
		snprintf(child_name_buf, sizeof(child_name_buf), "%s %c", name, (char)('A' + i));
		live.child[i] = obs_source_create_private(CHILD_ID, child_name_buf, NULL);
		assert_non_null(live.child[i]);
	}
	live.tr = private_source ? obs_source_create_private(id, name, NULL) : obs_source_create(id, name, NULL, NULL);
	assert_non_null(live.tr);
	signal_handler_t *sh = obs_source_get_signal_handler(live.tr);
	signal_handler_connect(sh, "transition_stop", on_stop, NULL);
	signal_handler_connect(sh, "transition_video_stop", on_video_stop, NULL);
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

/* The callback count, the stop signal on the transition and on the core's
 * handler, and the video stop signal. */
static void check_counts(long calls, long stops, long global_stops, long video_stops)
{
	assert_int_equal(os_atomic_load_long(&stop_calls), calls);
	assert_int_equal(os_atomic_load_long(&stop_signals), stops);
	assert_int_equal(os_atomic_load_long(&global_stop_signals), global_stops);
	assert_int_equal(os_atomic_load_long(&video_stop_signals), video_stops);
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

/* Wait until the transition's time is `t`. */
static bool time_is(obs_source_t *tr, float t)
{
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	while (obs_transition_get_time(tr) != t && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	return obs_transition_get_time(tr) == t;
}

/* Auto mode: a transition never started has no duration, so its time is
 * 1.0; early in a 600 s start it is just above 0; past a 100 ms start's end
 * it is 1.0, and stays 1.0 while the transition, rendered by nothing, keeps
 * running. */
static void get_time_auto(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition(TRANSITION_ID, "auto time", false);
	obs_source_t *b = live.child[1];
	obs_source_t *c = live.child[2];

	assert_true(frame_reaches(os_gettime_ns()));
	assert_true(obs_transition_get_time(tr) == 1.0f);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 600000, b));
	assert_true(frame_reaches(os_gettime_ns() + 100000000ULL));
	float t = obs_transition_get_time(tr);
	assert_true(t > 0.0f);
	assert_true(t < 0.01f);

	obs_transition_set(tr, live.child[0]);
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 100, c));
	assert_true(frame_reaches(os_gettime_ns() + 100000000ULL));
	assert_true(obs_transition_get_time(tr) == 1.0f);
	check_slots(tr, live.child[0], c, true);
	assert_true(frame_reaches(os_gettime_ns() + 100000000ULL));
	assert_true(obs_transition_get_time(tr) == 1.0f);
	/* Only the set that ended the long start signalled a stop. */
	check_counts(0, 1, 1, 0);
}

/* Manual mode: the start resets the manual value, so the time is 0.0; each
 * video tick snaps it to the manual time set (no torque); obs_transition_set()
 * ends the transition and resets the manual value, and the mode stays manual,
 * so the time is 0.0 again. */
static void get_time_manual(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition(TRANSITION_ID, "manual time", false);
	obs_source_t *b = live.child[1];

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, b));
	assert_true(obs_transition_get_time(tr) == 0.0f);

	obs_transition_set_manual_time(tr, 0.25f);
	assert_true(time_is(tr, 0.25f));
	obs_transition_set_manual_time(tr, 0.75f);
	assert_true(time_is(tr, 0.75f));
	check_slots(tr, live.child[0], b, true);

	obs_transition_set(tr, b);
	check_slots(tr, b, NULL, false);
	assert_true(frame_reaches(os_gettime_ns() + 100000000ULL));
	assert_true(obs_transition_get_time(tr) == 0.0f);
}

/* A force stop calls the callback and signals, and changes nothing else:
 * idle, the transition stays idle; running, it keeps running towards the
 * same destination. */
static void force_stop_calls_back_and_signals(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition(TRANSITION_ID, "force", false);
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];

	obs_transition_force_stop(tr);
	check_slots(tr, a, NULL, false);
	check_counts(1, 1, 1, 0);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 600000, b));
	obs_transition_force_stop(tr);
	obs_transition_force_stop(tr);
	check_slots(tr, a, b, true);
	check_counts(3, 3, 3, 0);
}

/* Without a transition_stop callback, only the signals. */
static void force_stop_without_callback(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition(BARE_TRANSITION_ID, "bare force", false);
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 600000, b));
	obs_transition_force_stop(tr);
	check_slots(tr, a, b, true);
	check_counts(0, 1, 1, 0);
}

/* A private transition: the callback and its own signal, nothing on the
 * core's handler. */
static void force_stop_private_transition(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition(TRANSITION_ID, "private force", true);

	obs_transition_force_stop(tr);
	check_slots(tr, live.child[0], NULL, false);
	check_counts(1, 1, 0, 0);
}

/* An auto transition that nothing shows, rendered directly past its end:
 * the video and the audio end together, slot B becomes slot A, slot B
 * empties, and the video stop, the stop and the callback come once. */
static void auto_stop_shifts_the_slots(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition(TRANSITION_ID, "auto stop", false);
	obs_source_t *a = live.child[0];
	obs_source_t *b = live.child[1];

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 50, b));
	assert_true(frame_reaches(os_gettime_ns() + 100000000ULL));
	check_slots(tr, a, b, true);
	check_counts(0, 0, 0, 0);

	obs_enter_graphics();
	bool transitioning = obs_transition_video_render_direct(tr, OBS_TRANSITION_SOURCE_B);
	obs_leave_graphics();
	assert_false(transitioning);
	check_slots(tr, b, NULL, false);
	check_counts(1, 1, 1, 1);
	obs_source_t *shown = obs_transition_get_active_source(tr);
	assert_ptr_equal(shown, b);
	obs_source_release(shown);

	/* Rendered again, nothing more stops. */
	obs_enter_graphics();
	transitioning = obs_transition_video_render_direct(tr, OBS_TRANSITION_SOURCE_B);
	obs_leave_graphics();
	assert_false(transitioning);
	check_slots(tr, b, NULL, false);
	check_counts(1, 1, 1, 1);
}

static void check_active(bool a, bool b, bool c)
{
	assert_true(obs_source_active(live.child[0]) == a);
	assert_true(obs_source_active(live.child[1]) == b);
	assert_true(obs_source_active(live.child[2]) == c);
}

/* Wait until the stop and the video stop have both been signalled: the
 * render thread signals the video stop after it lets go of the lock, and
 * the audio thread the stop, so either can come last. */
static bool stops(void)
{
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	while ((os_atomic_load_long(&stop_signals) < 1 || os_atomic_load_long(&video_stop_signals) < 1) &&
	       os_gettime_ns() < deadline)
		os_sleep_ms(5);
	return os_atomic_load_long(&stop_signals) >= 1 && os_atomic_load_long(&video_stop_signals) >= 1;
}

/* On an output channel the render thread ends the video, the audio thread
 * then ends the audio and stops the transition: slot B becomes slot A, and
 * the old slot A is no longer an active child. */
static void stop_on_channel_releases_the_old_child(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition(TRANSITION_ID, "channel stop", false);
	obs_source_t *b = live.child[1];

	obs_set_output_source(0, tr);
	live.on_channel = true;
	check_active(true, false, false);

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 50, b));
	assert_true(stops());
	check_slots(tr, b, NULL, false);
	check_active(false, true, false);
	check_counts(1, 1, 1, 1);
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

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "skipped: obs_reset_audio failed\n");
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	obs_register_source(&child_info);
	obs_register_source(&transition_info);
	obs_register_source(&bare_transition_info);
	signal_handler_connect(obs_get_signal_handler(), "source_transition_stop", on_global_stop, NULL);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up) {
		signal_handler_disconnect(obs_get_signal_handler(), "source_transition_stop", on_global_stop, NULL);
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(get_time_auto, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(get_time_manual, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(force_stop_calls_back_and_signals, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(force_stop_without_callback, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(force_stop_private_transition, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(auto_stop_shifts_the_slots, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(stop_on_channel_releases_the_old_child, case_setup, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
