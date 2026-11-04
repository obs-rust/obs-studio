/*
 * Characterization before the source audio settings of obs-source.c move to
 * their own file and are ported (#3619): obs_source_set_volume(),
 * obs_source_set_sync_offset(), obs_source_set_balance_value(),
 * obs_source_set_muted() and obs_source_set_monitoring_enabled().
 *
 * Other tests pin these in use. This one pins the signals: each one's name,
 * its value, what a handler may change, whether the getter has the new value
 * yet when the handler runs, and when a set sends no signal. A handler calls
 * the getter, so a setter that held audio_buf_mutex across a signal would
 * hang here.
 *
 * Creating a monitored source needs core audio, and core audio needs core
 * video, so the cases run under xvfb-run and skip without a display. No
 * monitoring device is used.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>

#define SOURCE_ID "test_source_audio_settings_source"
#define W 64
#define H 64
#define RATE 48000

static bool obs_up;

static const char *source_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "audio settings";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void source_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static struct obs_source_info source_info = {
	.id = SOURCE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = source_name,
	.create = source_create,
	.destroy = source_destroy,
};

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "skipped: no display\n");
		return 0;
	}
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = W,
		.base_height = H,
		.output_width = W,
		.output_height = H,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed\n");
		obs_shutdown();
		return 0;
	}
	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "skipped: obs_reset_audio failed\n");
		obs_shutdown();
		return 0;
	}

	obs_register_source(&source_info);
	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

static int case_setup(void **state)
{
	if (!obs_up) {
		skip();
		return 0;
	}
	*state = obs_source_create(SOURCE_ID, "settings", NULL, NULL);
	assert_non_null(*state);
	return 0;
}

static int case_teardown(void **state)
{
	if (*state)
		obs_source_release(*state);
	return 0;
}

/* ------------------------------------------------------------------------ */

struct float_signal {
	obs_source_t *source;
	const char *field;
	float (*get)(const obs_source_t *source);
	int count;
	double seen;
	float getter_in_handler;
	bool replace;
	double replacement;
};

static void float_signal_cb(void *param, calldata_t *data)
{
	struct float_signal *s = param;

	s->count++;
	s->seen = calldata_float(data, s->field);
	s->getter_in_handler = s->get(s->source);
	if (s->replace)
		calldata_set_float(data, s->field, s->replacement);
}

static void volume_is_stored_after_its_signal(void **state)
{
	obs_source_t *source = *state;
	struct float_signal s = {.source = source, .field = "volume", .get = obs_source_get_volume};
	signal_handler_t *handler = obs_source_get_signal_handler(source);

	signal_handler_connect(handler, "volume", float_signal_cb, &s);
	obs_source_set_volume(source, 0.5f);
	assert_int_equal(s.count, 1);
	assert_float_equal(s.seen, 0.5, 1e-6);
	assert_float_equal(s.getter_in_handler, 1.0f, 1e-6f);
	assert_float_equal(obs_source_get_volume(source), 0.5f, 1e-6f);

	/* a set to the same value signals too */
	obs_source_set_volume(source, 0.5f);
	assert_int_equal(s.count, 2);

	s.replace = true;
	s.replacement = 0.25;
	obs_source_set_volume(source, 0.75f);
	assert_float_equal(obs_source_get_volume(source), 0.25f, 1e-6f);
	signal_handler_disconnect(handler, "volume", float_signal_cb, &s);
}

static void balance_is_clamped_and_stored_after_its_signal(void **state)
{
	obs_source_t *source = *state;
	struct float_signal s = {.source = source, .field = "balance", .get = obs_source_get_balance_value};
	signal_handler_t *handler = obs_source_get_signal_handler(source);

	signal_handler_connect(handler, "audio_balance", float_signal_cb, &s);
	obs_source_set_balance_value(source, 0.75f);
	assert_int_equal(s.count, 1);
	assert_float_equal(s.seen, 0.75, 1e-6);
	assert_float_equal(s.getter_in_handler, 0.5f, 1e-6f);
	assert_float_equal(obs_source_get_balance_value(source), 0.75f, 1e-6f);

	/* the value out of range is clamped for the signal and in the store */
	obs_source_set_balance_value(source, 2.0f);
	assert_float_equal(s.seen, 1.0, 1e-6);
	assert_float_equal(obs_source_get_balance_value(source), 1.0f, 1e-6f);

	/* so is what a handler puts back */
	s.replace = true;
	s.replacement = -3.0;
	obs_source_set_balance_value(source, 0.25f);
	assert_float_equal(obs_source_get_balance_value(source), 0.0f, 1e-6f);
	signal_handler_disconnect(handler, "audio_balance", float_signal_cb, &s);
}

struct sync_signal {
	obs_source_t *source;
	int count;
	long long seen;
	int64_t getter_in_handler;
	bool replace;
	long long replacement;
};

static void sync_signal_cb(void *param, calldata_t *data)
{
	struct sync_signal *s = param;

	s->count++;
	s->seen = calldata_int(data, "offset");
	s->getter_in_handler = obs_source_get_sync_offset(s->source);
	if (s->replace)
		calldata_set_int(data, "offset", s->replacement);
}

static void sync_offset_is_stored_after_its_signal(void **state)
{
	obs_source_t *source = *state;
	struct sync_signal s = {.source = source};
	signal_handler_t *handler = obs_source_get_signal_handler(source);

	signal_handler_connect(handler, "audio_sync", sync_signal_cb, &s);
	obs_source_set_sync_offset(source, 1000);
	assert_int_equal(s.count, 1);
	assert_int_equal(s.seen, 1000);
	assert_int_equal(s.getter_in_handler, 0);
	assert_int_equal(obs_source_get_sync_offset(source), 1000);

	s.replace = true;
	s.replacement = -2000;
	obs_source_set_sync_offset(source, 3000);
	assert_int_equal(obs_source_get_sync_offset(source), -2000);
	signal_handler_disconnect(handler, "audio_sync", sync_signal_cb, &s);
}

struct mute_signal {
	obs_source_t *source;
	int count;
	bool seen;
	bool getter_in_handler;
};

static void mute_signal_cb(void *param, calldata_t *data)
{
	struct mute_signal *s = param;

	s->count++;
	s->seen = calldata_bool(data, "muted");
	s->getter_in_handler = obs_source_muted(s->source);
}

static void mute_is_stored_before_its_signal(void **state)
{
	obs_source_t *source = *state;
	struct mute_signal s = {.source = source};
	signal_handler_t *handler = obs_source_get_signal_handler(source);

	signal_handler_connect(handler, "mute", mute_signal_cb, &s);
	obs_source_set_muted(source, true);
	assert_int_equal(s.count, 1);
	assert_true(s.seen);
	assert_true(s.getter_in_handler);

	/* a set to the same value signals too */
	obs_source_set_muted(source, true);
	assert_int_equal(s.count, 2);

	obs_source_set_muted(source, false);
	assert_false(s.seen);
	assert_false(obs_source_muted(source));
	signal_handler_disconnect(handler, "mute", mute_signal_cb, &s);
}

struct monitor_signals {
	obs_source_t *source;
	int count;
	int monitor_at;
	int type_at;
	bool monitor;
	long long type;
	bool getter_in_monitor;
	bool getter_in_type;
};

static void monitor_cb(void *param, calldata_t *data)
{
	struct monitor_signals *s = param;

	s->monitor_at = ++s->count;
	s->monitor = calldata_bool(data, "monitor");
	s->getter_in_monitor = obs_source_get_monitoring_enabled(s->source);
}

static void audio_monitoring_cb(void *param, calldata_t *data)
{
	struct monitor_signals *s = param;

	s->type_at = ++s->count;
	s->type = calldata_int(data, "type");
	s->getter_in_type = obs_source_get_monitoring_enabled(s->source);
}

static void monitoring_is_stored_after_both_signals(void **state)
{
	obs_source_t *source = *state;
	struct monitor_signals s = {.source = source};
	signal_handler_t *handler = obs_source_get_signal_handler(source);

	signal_handler_connect(handler, "monitor", monitor_cb, &s);
	signal_handler_connect(handler, "audio_monitoring", audio_monitoring_cb, &s);

	obs_source_set_monitoring_enabled(source, true);
	assert_int_equal(s.monitor_at, 1);
	assert_int_equal(s.type_at, 2);
	assert_true(s.monitor);
	assert_int_equal(s.type, OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT);
	assert_false(s.getter_in_monitor);
	assert_false(s.getter_in_type);
	assert_true(obs_source_get_monitoring_enabled(source));

	/* a set to the same value does not signal */
	obs_source_set_monitoring_enabled(source, true);
	assert_int_equal(s.count, 2);

	/* the deprecated type of a muted, monitored source is monitor-only */
	obs_source_set_monitoring_enabled(source, false);
	assert_int_equal(s.type, OBS_MONITORING_TYPE_NONE);
	obs_source_set_muted(source, true);
	obs_source_set_monitoring_enabled(source, true);
	assert_int_equal(s.type, OBS_MONITORING_TYPE_MONITOR_ONLY);
	obs_source_set_monitoring_enabled(source, false);

	signal_handler_disconnect(handler, "monitor", monitor_cb, &s);
	signal_handler_disconnect(handler, "audio_monitoring", audio_monitoring_cb, &s);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(volume_is_stored_after_its_signal, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(balance_is_clamped_and_stored_after_its_signal, case_setup,
						case_teardown),
		cmocka_unit_test_setup_teardown(sync_offset_is_stored_after_its_signal, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(mute_is_stored_before_its_signal, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(monitoring_is_stored_after_both_signals, case_setup, case_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
