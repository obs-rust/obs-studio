/*
 * A source saved before 33.0 as "Monitor Only" must stay out of the stream
 * and recordings (#2444).
 *
 * Before 33.0 a source had three monitoring types: off, Monitor Only and
 * Monitor and Output. 33.0 keeps an on/off switch, and on means monitor and
 * output; the monitor ignores mute, so a monitor-only source is now
 * monitoring on plus muted. The load migration turned every type but off
 * into monitoring on and left the mute alone, so an unmuted Monitor Only
 * source started playing into every output. Now Monitor Only loads as
 * monitoring on and muted, Monitor and Output as monitoring on and unmuted,
 * and off as off; a source saved by 33.0 or later loads as it was saved.
 *
 * A thread delivers 10 ms packets of a constant on the left channel,
 * stamped with the system clock, like a live capture. The source is loaded
 * from a scene collection entry, put on the program, and the mixed audio
 * libobs hands to outputs (obs_add_raw_audio_callback) must carry the tone,
 * or stay silent for a monitor-only source. The audio the monitor is fed
 * (the source's audio capture callbacks, which get it muted or not) must
 * carry the tone in every case. Mixing needs core video, so the cases run
 * under xvfb-run and skip without a display. No monitoring device is used.
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

#define SOURCE_ID "test_source_monitor_only_load_capture"

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

#define V32_2_2 MAKE_SEMANTIC_VERSION(32, 2, 2)
#define V33_0_0 MAKE_SEMANTIC_VERSION(33, 0, 0)

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
	return "monitor only capture";
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
/* loading and listening */

static obs_source_t *live;

static void drop_live(void)
{
	if (!live)
		return;
	obs_set_output_source(0, NULL);
	obs_source_remove_audio_capture_callback(live, monitor_feed, NULL);
	obs_source_remove(live);
	obs_source_release(live);
	live = NULL;
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	drop_live();
	return 0;
}

struct saved {
	const char *what;
	uint32_t prev_ver;
	int monitoring_type;    /* -1: not saved */
	int monitoring_enabled; /* -1: not saved */
	bool muted;
};

struct expected {
	bool monitoring;
	bool muted;
	bool in_program;
};

/* Loads the entry, puts it on the program and listens. Reports every
 * difference before returning false. */
static bool expect_load(const struct saved *s, const struct expected *e)
{
	char name[64];
	snprintf(name, sizeof(name), "monitor only %d", source_count++);

	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "id", SOURCE_ID);
	obs_data_set_string(data, "name", name);
	obs_data_set_int(data, "prev_ver", s->prev_ver);
	if (s->monitoring_type >= 0)
		obs_data_set_int(data, "monitoring_type", s->monitoring_type);
	if (s->monitoring_enabled >= 0)
		obs_data_set_bool(data, "monitoring_enabled", s->monitoring_enabled != 0);
	obs_data_set_bool(data, "muted", s->muted);
	live = obs_load_source(data);
	obs_data_release(data);
	assert_non_null(live);
	obs_source_add_audio_capture_callback(live, monitor_feed, NULL);

	/* what is left of the previous case leaves the mix first */
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
		if (e->in_program && program >= WANT_FRAMES && monitor >= WANT_FRAMES)
			break;
		if (!e->in_program && program)
			break;
		if (!e->in_program && heard_monitor_at >= 0 && waited - heard_monitor_at >= LISTEN_MS)
			break;
	}
	obs_set_output_source(0, NULL);

	bool monitoring = obs_source_get_monitoring_enabled(live);
	bool muted = obs_source_muted(live);
	fprintf(stderr, "%s: monitoring %s, muted %s, %zu program frames and %zu monitor feed frames carry the tone\n",
		s->what, monitoring ? "on" : "off", muted ? "yes" : "no", program, monitor);

	bool ok = true;
	if (monitoring != e->monitoring) {
		fprintf(stderr, "%s: monitoring %s, expected %s\n", s->what, monitoring ? "on" : "off",
			e->monitoring ? "on" : "off");
		ok = false;
	}
	if (muted != e->muted) {
		fprintf(stderr, "%s: muted %s, expected %s\n", s->what, muted ? "yes" : "no", e->muted ? "yes" : "no");
		ok = false;
	}
	if (e->in_program && program < WANT_FRAMES) {
		fprintf(stderr, "%s: only %zu of %d program frames carry the tone\n", s->what, program, WANT_FRAMES);
		ok = false;
	}
	if (!e->in_program && program) {
		fprintf(stderr, "%s: the tone reached the program mix (%zu frames); it must only be monitored\n",
			s->what, program);
		ok = false;
	}
	if (monitor < WANT_FRAMES) {
		fprintf(stderr, "%s: only %zu of %d monitor feed frames carry the tone\n", s->what, monitor,
			WANT_FRAMES);
		ok = false;
	}

	drop_live();
	return ok;
}

/* ------------------------------------------------------------------------ */
/* cases */

/* the reported case: an unmuted Monitor Only source from 32.x */
static void legacy_monitor_only_stays_out_of_program(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	const struct saved s = {"32.2.2 Monitor Only, unmuted", V32_2_2, 1, -1, false};
	const struct expected e = {.monitoring = true, .muted = true, .in_program = false};
	if (!expect_load(&s, &e))
		fail();
}

/* the other pre-33 types load as they sounded before 33.0 */
static void legacy_other_types_as_before(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	const struct {
		struct saved s;
		struct expected e;
	} cases[] = {
		{{"32.2.2 Monitor Only, muted", V32_2_2, 1, -1, true}, {true, true, false}},
		{{"32.2.2 Monitor and Output, unmuted", V32_2_2, 2, -1, false}, {true, false, true}},
		{{"32.2.2 off, unmuted", V32_2_2, 0, -1, false}, {false, false, true}},
		{{"32.2.2 no type saved, unmuted", V32_2_2, -1, -1, false}, {false, false, true}},
	};

	bool ok = true;
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++)
		ok &= expect_load(&cases[i].s, &cases[i].e);
	if (!ok)
		fail();
}

/* a source saved by 33.0 loads as it was saved, whatever type it carries */
static void saved_by_33_unchanged(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	const struct {
		struct saved s;
		struct expected e;
	} cases[] = {
		/* what 33.0 saves for monitoring on */
		{{"33.0.0 monitoring on, unmuted", V33_0_0, 2, 1, false}, {true, false, true}},
		{{"33.0.0 monitoring on, muted", V33_0_0, 2, 1, true}, {true, true, false}},
		{{"33.0.0 monitoring off, unmuted", V33_0_0, 0, 0, false}, {false, false, true}},
		/* a stale Monitor Only type next to the 33.0 switch is ignored */
		{{"33.0.0 monitoring on, type 1, unmuted", V33_0_0, 1, 1, false}, {true, false, true}},
	};

	bool ok = true;
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++)
		ok &= expect_load(&cases[i].s, &cases[i].e);
	if (!ok)
		fail();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(legacy_monitor_only_stays_out_of_program, case_teardown),
		cmocka_unit_test_teardown(legacy_other_types_as_before, case_teardown),
		cmocka_unit_test_teardown(saved_by_33_unchanged, case_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
