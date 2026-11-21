/*
 * A Stinger transition can be set to Monitor Only again
 * (upstream obsproject/obs-studio#14144).
 *
 * Before 33.0 the Stinger's "Audio Monitoring" was a list: Monitor Off,
 * Monitor Only (mute output) and Monitor and Output. 33.0 made it one
 * checkbox, and a source with monitoring on is monitored and output at once.
 * A source gets Monitor Only back by muting it in the mixer, but a transition
 * has no mixer entry, so a Stinger made or edited in 33.0 could only be off
 * or Monitor and Output: its clip played into the stream and recordings
 * whenever it was monitored.
 *
 * The "monitor_only" checkbox, shown while monitoring is on, brings the
 * choice back: the clip is monitored and muted, which 33.0 monitors and
 * keeps out of every output, as a 32.x Monitor Only transition is loaded
 * (obs-rust/obs-studio#2638).
 *
 * The cases load the built obs-transitions module and look at the clip
 * source the Stinger holds once the video thread has applied its settings. Creating a Stinger needs core video, so they run
 * under xvfb-run and skip without a display.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <obs-module.h>
#include <util/platform.h>
#include <util/threading.h>

#define STINGER_ID "obs_stinger_transition"

static bool obs_up;
static obs_module_t *transitions;

static int group_setup(void **state)
{
	(void)state;

	const char *display = getenv("DISPLAY");
	if (!display || !*display)
		return 0;

	if (!obs_startup("en-US", NULL, NULL))
		return 0;

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		obs_shutdown();
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = 48000, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		obs_shutdown();
		return 0;
	}

	if (obs_open_module(&transitions, TRANSITIONS_MODULE_PATH, TRANSITIONS_DATA_PATH) != MODULE_SUCCESS ||
	    !obs_init_module(transitions)) {
		fprintf(stderr, "could not load the obs-transitions module\n");
		obs_shutdown();
		return -1;
	}

	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_shutdown();
		obs_up = false;
	}
	return 0;
}

static void need_obs(void)
{
	if (!obs_up)
		skip();
}

/* ------------------------------------------------------------------------ */
/* the clip source */

struct find_clip {
	obs_source_t *clip;
	unsigned count;
};

static void find_clip_cb(obs_source_t *parent, obs_source_t *child, void *param)
{
	(void)parent;
	struct find_clip *f = param;
	const char *name = obs_source_get_name(child);

	/* The Stinger names its clip source "<transition name> (Stinger)". */
	if (name && strcmp(name, "stinger (Stinger)") == 0) {
		f->clip = child;
		f->count++;
	}
}

struct clip_state {
	bool monitoring;
	bool muted;
};

static void updated_cb(void *param, calldata_t *cd)
{
	(void)cd;
	os_atomic_inc_long(param);
}

/* Creates a Stinger from `settings` and reads its clip source's monitoring
 * and mute. */
static struct clip_state stinger_clip(obs_data_t *settings)
{
	obs_data_set_string(settings, "path", "/nonexistent/obs-test-stinger.webm");

	obs_source_t *stinger = obs_source_create_private(STINGER_ID, "stinger", settings);
	assert_non_null(stinger);

	/* A video source applies its settings on a video tick and signals
	 * "update" after the update ran, clip, monitoring and mute included.
	 * Ask for one more update after connecting, so one is seen, then wait
	 * until no update has run for 100 ms: a pending one would replace the
	 * clip while it is read. */
	volatile long updates = 0;
	signal_handler_t *sh = obs_source_get_signal_handler(stinger);
	signal_handler_connect(sh, "update", updated_cb, (void *)&updates);
	obs_source_update(stinger, NULL);

	long seen = -1;
	for (int i = 0; i < 50; i++) {
		os_sleep_ms(100);
		long now = os_atomic_load_long(&updates);
		if (now > 0 && now == seen)
			break;
		seen = now;
	}
	signal_handler_disconnect(sh, "update", updated_cb, (void *)&updates);
	assert_true(os_atomic_load_long(&updates) > 0);

	struct find_clip f = {0};
	obs_source_enum_full_tree(stinger, find_clip_cb, &f);
	assert_int_equal(f.count, 1);

	struct clip_state st = {
		.monitoring = obs_source_get_monitoring_enabled(f.clip),
		.muted = obs_source_muted(f.clip),
	};

	obs_source_release(stinger);
	return st;
}

/* ------------------------------------------------------------------------ */
/* cases */

/* Monitoring on and Monitor Only on: the clip is monitored and muted, so it
 * stays out of the stream and recordings. */
static void test_monitor_only_mutes_the_clip(void **state)
{
	(void)state;
	need_obs();

	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "enable_monitoring", true);
	obs_data_set_bool(settings, "monitor_only", true);
	struct clip_state st = stinger_clip(settings);
	obs_data_release(settings);

	assert_true(st.monitoring);
	assert_true(st.muted);
}

/* Monitoring on without Monitor Only is Monitor and Output: not muted. */
static void test_monitoring_alone_still_outputs(void **state)
{
	(void)state;
	need_obs();

	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "enable_monitoring", true);
	struct clip_state st = stinger_clip(settings);
	obs_data_release(settings);

	assert_true(st.monitoring);
	assert_false(st.muted);
}

/* Monitor Only has no effect while monitoring is off: muting a clip nobody
 * monitors would silence the transition everywhere. */
static void test_monitor_only_needs_monitoring(void **state)
{
	(void)state;
	need_obs();

	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "enable_monitoring", false);
	obs_data_set_bool(settings, "monitor_only", true);
	struct clip_state st = stinger_clip(settings);
	obs_data_release(settings);

	assert_false(st.monitoring);
	assert_false(st.muted);
}

/* A Stinger saved by 32.x as Monitor Only still loads monitored and muted
 * (#2638). */
static void test_old_monitor_only_still_loads_muted(void **state)
{
	(void)state;
	need_obs();

	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "audio_monitoring", 1);
	struct clip_state st = stinger_clip(settings);
	obs_data_release(settings);

	assert_true(st.monitoring);
	assert_true(st.muted);
}

/* The properties offer Monitor Only, shown only while monitoring is on. */
static void test_properties_offer_monitor_only(void **state)
{
	(void)state;
	need_obs();

	obs_properties_t *props = obs_get_source_properties(STINGER_ID);
	assert_non_null(props);

	obs_property_t *monitor_only = obs_properties_get(props, "monitor_only");
	obs_property_t *enable = obs_properties_get(props, "enable_monitoring");
	assert_non_null(monitor_only);
	assert_non_null(enable);
	assert_int_equal(obs_property_get_type(monitor_only), OBS_PROPERTY_BOOL);

	obs_data_t *settings = obs_data_create();

	obs_data_set_bool(settings, "enable_monitoring", false);
	obs_property_modified(enable, settings);
	assert_false(obs_property_visible(monitor_only));

	obs_data_set_bool(settings, "enable_monitoring", true);
	obs_property_modified(enable, settings);
	assert_true(obs_property_visible(monitor_only));

	obs_data_release(settings);
	obs_properties_destroy(props);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_monitor_only_mutes_the_clip),
		cmocka_unit_test(test_monitoring_alone_still_outputs),
		cmocka_unit_test(test_monitor_only_needs_monitoring),
		cmocka_unit_test(test_old_monitor_only_still_loads_muted),
		cmocka_unit_test(test_properties_offer_monitor_only),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
