/*
 * The deprecated obs_source_set_monitoring_type() and
 * obs_source_get_monitoring_type() must keep their three types on 33.0
 * (#2447, the API side of #2444).
 *
 * 33.0 keeps an on/off monitoring switch, and on means monitor and output;
 * the monitor ignores mute, so Monitor Only is monitoring on plus muted, as
 * a pre-33.0 Monitor Only source loads (#2445). The setter turned every type
 * but none into monitoring on and left the mute alone, so a source a plugin,
 * script or obs-websocket set to Monitor Only played into the stream and
 * recordings, and the getter reported Monitor and Output for it.
 *
 * Now Monitor Only mutes the source and turns monitoring on, Monitor and
 * Output unmutes it and turns monitoring on, and None turns monitoring off
 * and keeps the mute, except that it undoes a mute Monitor Only set when
 * nothing has set the mute since. The getter and the "audio_monitoring"
 * signal report a monitored, muted source as Monitor Only.
 *
 * A thread delivers 10 ms packets of a constant on the left channel,
 * stamped with the system clock, like a live capture. The source is put on
 * the program, and the mixed audio libobs hands to outputs
 * (obs_add_raw_audio_callback) must carry the tone, or stay silent while the
 * source is muted. The audio the monitor is fed (the source's audio capture
 * callbacks, which get it muted or not) must carry the tone whenever
 * monitoring is on. Mixing needs core video, so the cases run under xvfb-run
 * and skip without a display. No monitoring device is used.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>

#define SOURCE_ID "test_source_monitoring_type_capture"

#define W 64
#define H 64
#define RATE 48000
#define MSEC 1000000ULL
#define PACKET_FRAMES 480 /* 10 ms */
#define PACKET_NS (10 * MSEC)
#define LEVEL 0.25f
#define TOLERANCE 1e-6f
/* frames of the tone to see before a case passes */
#define WANT_FRAMES 4096
#define WAIT_MS 4000
/* how long a silent case listens, once the monitor feed has the tone */
#define LISTEN_MS 500

static bool obs_up;
static int source_count;

/* ------------------------------------------------------------------------ */
/* the capture */

struct capture {
	obs_source_t *source;
	pthread_t thread;
	bool thread_started;
	volatile bool stop;
};

static void *capture_thread(void *param)
{
	struct capture *c = param;
	float left[PACKET_FRAMES];
	static float right[PACKET_FRAMES];
	for (size_t i = 0; i < PACKET_FRAMES; i++)
		left[i] = LEVEL;

	os_sleep_ms(50);
	uint64_t t0 = os_gettime_ns();
	for (uint64_t n = 0; !c->stop; n++) {
		uint64_t ts = t0 + n * PACKET_NS;
		os_sleepto_ns(ts + PACKET_NS);

		struct obs_source_audio audio = {
			.data = {(const uint8_t *)left, (const uint8_t *)right},
			.frames = PACKET_FRAMES,
			.speakers = SPEAKERS_STEREO,
			.format = AUDIO_FORMAT_FLOAT_PLANAR,
			.samples_per_sec = RATE,
			.timestamp = ts,
		};
		obs_source_output_audio(c->source, &audio);
	}
	return NULL;
}

static const char *capture_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "monitoring type capture";
}

static void *capture_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	struct capture *c = bzalloc(sizeof(*c));
	c->source = source;
	if (pthread_create(&c->thread, NULL, capture_thread, c) == 0)
		c->thread_started = true;
	else
		fprintf(stderr, "could not start the capture thread\n");
	return c;
}

static void capture_destroy(void *data)
{
	struct capture *c = data;
	if (c->thread_started) {
		c->stop = true;
		pthread_join(c->thread, NULL);
	}
	bfree(c);
}

static struct obs_source_info capture_info = {
	.id = SOURCE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = capture_name,
	.create = capture_create,
	.destroy = capture_destroy,
};

/* ------------------------------------------------------------------------ */
/* what is heard: the program mix and the monitor feed */

static pthread_mutex_t heard_mutex;
static size_t program_frames; /* program frames carrying the tone */
static size_t monitor_frames; /* monitor feed frames carrying the tone */

static size_t count_tone(const float *left, uint32_t frames)
{
	size_t n = 0;
	for (uint32_t i = 0; i < frames; i++)
		if (fabsf(left[i] - LEVEL) <= TOLERANCE)
			n++;
	return n;
}

static void raw_audio(void *param, size_t mix_idx, struct audio_data *data)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(mix_idx);
	size_t n = count_tone((const float *)data->data[0], data->frames);
	pthread_mutex_lock(&heard_mutex);
	program_frames += n;
	pthread_mutex_unlock(&heard_mutex);
}

static void monitor_feed(void *param, obs_source_t *source, const struct audio_data *data, bool muted)
{
	UNUSED_PARAMETER(param);
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(muted);
	size_t n = count_tone((const float *)data->data[0], data->frames);
	pthread_mutex_lock(&heard_mutex);
	monitor_frames += n;
	pthread_mutex_unlock(&heard_mutex);
}

static void read_heard(size_t *program, size_t *monitor)
{
	pthread_mutex_lock(&heard_mutex);
	*program = program_frames;
	*monitor = monitor_frames;
	pthread_mutex_unlock(&heard_mutex);
}

static void reset_heard(void)
{
	pthread_mutex_lock(&heard_mutex);
	program_frames = 0;
	monitor_frames = 0;
	pthread_mutex_unlock(&heard_mutex);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	/* no monitoring device: the monitor never plays anything */
	setenv("PULSE_SERVER", "unix:/nonexistent-monitoring-type", 1);

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

	obs_register_source(&capture_info);
	pthread_mutex_init(&heard_mutex, NULL);
	obs_add_raw_audio_callback(0, NULL, raw_audio, NULL);

	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up) {
		obs_remove_raw_audio_callback(0, raw_audio, NULL);
		obs_shutdown();
		pthread_mutex_destroy(&heard_mutex);
		obs_up = false;
	}
	return 0;
}

/* ------------------------------------------------------------------------ */
/* the source and the "audio_monitoring" signal */

static obs_source_t *live;
static long long signalled_type = -1; /* the type the last signal carried */

static void audio_monitoring(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	signalled_type = calldata_int(cd, "type");
}

static int case_setup(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		return 0;
	char name[64];
	snprintf(name, sizeof(name), "monitoring type %d", source_count++);
	live = obs_source_create(SOURCE_ID, name, NULL, NULL);
	if (!live)
		return -1;
	obs_source_add_audio_capture_callback(live, monitor_feed, NULL);
	signal_handler_connect(obs_source_get_signal_handler(live), "audio_monitoring", audio_monitoring, NULL);
	signalled_type = -1;
	return 0;
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!live)
		return 0;
	obs_set_output_source(0, NULL);
	signal_handler_disconnect(obs_source_get_signal_handler(live), "audio_monitoring", audio_monitoring, NULL);
	obs_source_remove_audio_capture_callback(live, monitor_feed, NULL);
	obs_source_remove(live);
	obs_source_release(live);
	live = NULL;
	return 0;
}

/* deprecated, but plugins, scripts and obs-websocket still call them */
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
static void set_type(enum obs_monitoring_type type)
{
	obs_source_set_monitoring_type(live, type);
}

static enum obs_monitoring_type get_type(void)
{
	return obs_source_get_monitoring_type(live);
}
#pragma GCC diagnostic pop

static const char *type_name(long long type)
{
	switch (type) {
	case OBS_MONITORING_TYPE_NONE:
		return "none";
	case OBS_MONITORING_TYPE_MONITOR_ONLY:
		return "monitor only";
	case OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT:
		return "monitor and output";
	case -1:
		return "no signal";
	}
	return "not a type";
}

struct expected {
	enum obs_monitoring_type type; /* what the getter reports */
	bool monitoring;
	bool muted;
};

/* Checks the source's state, puts it on the program and listens. Reports
 * every difference before returning false. The tone is in the program
 * exactly when the source is not muted. */
static bool expect(const char *what, const struct expected *e)
{
	bool ok = true;
	bool in_program = !e->muted;

	enum obs_monitoring_type type = get_type();
	if (type != e->type) {
		fprintf(stderr, "%s: the getter reports %s, expected %s\n", what, type_name(type), type_name(e->type));
		ok = false;
	}

	/* what is left of the previous step leaves the mix first */
	obs_set_output_source(0, NULL);
	os_sleep_ms(100);
	reset_heard();
	obs_set_output_source(0, live);

	size_t program = 0, monitor = 0;
	int heard_monitor_at = -1;
	for (int waited = 0; waited < WAIT_MS; waited += 20) {
		os_sleep_ms(20);
		read_heard(&program, &monitor);
		if (heard_monitor_at < 0 && monitor >= WANT_FRAMES)
			heard_monitor_at = waited;
		if (in_program && program >= WANT_FRAMES && monitor >= WANT_FRAMES)
			break;
		if (!in_program && program)
			break;
		if (!in_program && heard_monitor_at >= 0 && waited - heard_monitor_at >= LISTEN_MS)
			break;
	}
	obs_set_output_source(0, NULL);

	bool monitoring = obs_source_get_monitoring_enabled(live);
	bool muted = obs_source_muted(live);
	fprintf(stderr, "%s: reports %s, monitoring %s, muted %s, %zu program frames carry the tone\n", what,
		type_name(type), monitoring ? "on" : "off", muted ? "yes" : "no", program);

	if (monitoring != e->monitoring) {
		fprintf(stderr, "%s: monitoring %s, expected %s\n", what, monitoring ? "on" : "off",
			e->monitoring ? "on" : "off");
		ok = false;
	}
	if (muted != e->muted) {
		fprintf(stderr, "%s: muted %s, expected %s\n", what, muted ? "yes" : "no", e->muted ? "yes" : "no");
		ok = false;
	}
	if (in_program && program < WANT_FRAMES) {
		fprintf(stderr, "%s: only %zu of %d program frames carry the tone\n", what, program, WANT_FRAMES);
		ok = false;
	}
	if (!in_program && program) {
		fprintf(stderr, "%s: the tone reached the program mix (%zu frames)\n", what, program);
		ok = false;
	}
	/* the monitor feed gets the audio whether or not monitoring is on */
	if (monitor < WANT_FRAMES) {
		fprintf(stderr, "%s: only %zu of %d monitor feed frames carry the tone\n", what, monitor, WANT_FRAMES);
		ok = false;
	}
	return ok;
}

/* the type the last "audio_monitoring" signal carried */
static bool expect_signal(const char *what, long long type)
{
	if (signalled_type == type)
		return true;
	fprintf(stderr, "%s: the audio_monitoring signal carried %s, expected %s\n", what, type_name(signalled_type),
		type_name(type));
	return false;
}

static const struct expected MONITOR_ONLY = {OBS_MONITORING_TYPE_MONITOR_ONLY, true, true};
static const struct expected MONITOR_AND_OUTPUT = {OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT, true, false};
static const struct expected NONE_UNMUTED = {OBS_MONITORING_TYPE_NONE, false, false};
static const struct expected NONE_MUTED = {OBS_MONITORING_TYPE_NONE, false, true};

/* ------------------------------------------------------------------------ */
/* cases */

/* #2447: Monitor Only mutes, keeps the tone out of the program, reads back
 * and is signalled as Monitor Only */
static void monitor_only_stays_out_of_program(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	set_type(OBS_MONITORING_TYPE_MONITOR_ONLY);
	bool ok = expect_signal("Monitor Only", OBS_MONITORING_TYPE_MONITOR_ONLY);
	ok &= expect("Monitor Only", &MONITOR_ONLY);
	if (!ok)
		fail();
}

/* the other two types read back, sound and are signalled as they say */
static void other_types_round_trip(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	bool ok = true;
	set_type(OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT);
	ok &= expect_signal("Monitor and Output", OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT);
	ok &= expect("Monitor and Output", &MONITOR_AND_OUTPUT);
	set_type(OBS_MONITORING_TYPE_NONE);
	ok &= expect_signal("then None", OBS_MONITORING_TYPE_NONE);
	ok &= expect("then None", &NONE_UNMUTED);
	if (!ok)
		fail();
}

/* Leaving Monitor Only undoes its mute: Monitor and Output unmutes, and so
 * does None, which plays the source into the outputs as before 33.0. */
static void leaving_monitor_only_undoes_its_mute(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	bool ok = true;
	set_type(OBS_MONITORING_TYPE_MONITOR_ONLY);
	ok &= expect("Monitor Only", &MONITOR_ONLY);
	set_type(OBS_MONITORING_TYPE_NONE);
	ok &= expect("Monitor Only, then None", &NONE_UNMUTED);
	set_type(OBS_MONITORING_TYPE_MONITOR_ONLY);
	set_type(OBS_MONITORING_TYPE_MONITOR_ONLY);
	ok &= expect("Monitor Only twice", &MONITOR_ONLY);
	set_type(OBS_MONITORING_TYPE_NONE);
	ok &= expect("Monitor Only twice, then None", &NONE_UNMUTED);
	set_type(OBS_MONITORING_TYPE_MONITOR_ONLY);
	set_type(OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT);
	ok &= expect("Monitor Only, then Monitor and Output", &MONITOR_AND_OUTPUT);
	if (!ok)
		fail();
}

/* None keeps a mute set by anything but Monitor Only: one from before
 * Monitor Only, and one set again after Monitor Only's was undone by hand */
static void none_keeps_other_mutes(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	bool ok = true;
	obs_source_set_muted(live, true);
	set_type(OBS_MONITORING_TYPE_NONE);
	ok &= expect("muted, None", &NONE_MUTED);
	set_type(OBS_MONITORING_TYPE_MONITOR_ONLY);
	ok &= expect("muted, Monitor Only", &MONITOR_ONLY);
	set_type(OBS_MONITORING_TYPE_NONE);
	ok &= expect("muted, Monitor Only, then None", &NONE_MUTED);

	obs_source_set_muted(live, false);
	set_type(OBS_MONITORING_TYPE_MONITOR_ONLY);
	obs_source_set_muted(live, false);
	ok &= expect("Monitor Only, unmuted", &MONITOR_AND_OUTPUT);
	set_type(OBS_MONITORING_TYPE_NONE);
	ok &= expect("Monitor Only, unmuted, then None", &NONE_UNMUTED);

	set_type(OBS_MONITORING_TYPE_MONITOR_ONLY);
	obs_source_set_muted(live, true);
	set_type(OBS_MONITORING_TYPE_NONE);
	ok &= expect("Monitor Only, muted again, then None", &NONE_MUTED);
	if (!ok)
		fail();
}

/* the getter and the signal report the 33.0 state however it was set:
 * monitoring on and muted is Monitor Only, on and unmuted is Monitor and
 * Output */
static void getter_reports_monitoring_and_mute(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	bool ok = true;
	obs_source_set_muted(live, true);
	obs_source_set_monitoring_enabled(live, true);
	ok &= expect_signal("muted, monitoring on", OBS_MONITORING_TYPE_MONITOR_ONLY);
	ok &= expect("muted, monitoring on", &MONITOR_ONLY);
	obs_source_set_muted(live, false);
	ok &= expect("monitoring on, unmuted", &MONITOR_AND_OUTPUT);
	obs_source_set_monitoring_enabled(live, false);
	ok &= expect_signal("monitoring off", OBS_MONITORING_TYPE_NONE);
	ok &= expect("monitoring off", &NONE_UNMUTED);
	obs_source_set_monitoring_enabled(live, true);
	ok &= expect_signal("unmuted, monitoring on", OBS_MONITORING_TYPE_MONITOR_AND_OUTPUT);
	if (!ok)
		fail();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(monitor_only_stays_out_of_program, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(other_types_round_trip, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(leaving_monitor_only_undoes_its_mute, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(none_keeps_other_mutes, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(getter_reports_monitoring_and_mute, case_setup, case_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
