/* Characterize source_muted() and get_source_volume() (#5853), the two
 * push-to-mute and push-to-talk mute steps of a source, through the real
 * libobs (hybrid and the C reference):
 *
 * - source_muted() is the audio output path's mute check: every packet
 *   obs_source_output_audio() takes reaches the audio capture callbacks with
 *   its answer. It reads the user's flags (user_push_to_mute_pressed,
 *   user_push_to_talk_pressed, user_muted) at os_gettime_ns().
 * - get_source_volume() is the volume the audio mix thread applies to a
 *   source in the mix: 0 while it is muted, else its volume. It reads the
 *   mix thread's flags (push_to_mute_pressed, push_to_talk_pressed, muted) at
 *   the source's audio timestamp. The program mix (obs_add_raw_audio_callback)
 *   carries the source at that volume or is silent.
 *
 * Both write a key's stop time (the moment plus its delay) while the key is
 * enabled and pressed, before they read it back: a pressed key holds, and a
 * released one holds strictly before its stop time. Then a disabled source
 * is muted, then a muted one, then an active push-to-mute key mutes and an
 * inactive push-to-talk key mutes. Each reads only its own set of flags:
 * user_muted does not mute the mix and muted does not mute the capture, and
 * the same holds for the pressed flags. A key that is not enabled is
 * neither written nor obeyed. A disabled source still has its stop time
 * written (the writes come first).
 *
 * The test sets the fields directly (obs-internal.h), each before the source
 * reaches the path that reads it: the capture cases on a source that is
 * never in the mix, the mix cases before the source is put on the program.
 * The two share the stop times, so a mix case never presses a user key.
 * Mixing needs core video, so the cases run (under xvfb-run) on Linux and
 * skip without a display. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

#include <obs-internal.h>
#include <util/platform.h>
#include <util/threading.h>

#define CAPTURE_ID "test_source_muted_volume_capture"
#define LIVE_ID "test_source_muted_volume_live"

#define W 64
#define H 64
#define RATE 48000
#define MSEC 1000000ULL
#define SEC (1000 * MSEC)
#define FRAMES 64
#define PACKET_FRAMES 480 /* 10 ms */
#define PACKET_NS (10 * MSEC)
#define LEVEL 0.004f
#define VOLUME 0.5f
#define TOLERANCE 1e-6f
/* frames of the source at its volume to see before a mix case passes */
#define WANT_FRAMES 2048
#define WAIT_MS 4000
/* how long a silent mix case listens */
#define LISTEN_MS 500
#define DELAY_MS 300ULL

static bool obs_up;
static int source_count;

static void quiet_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (level > LOG_WARNING)
		return;
	vfprintf(stderr, format, args);
	fputc('\n', stderr);
}

/* ------------------------------------------------------------------------ */
/* the capture path: source_muted() */

static const char *capture_name(void *unused)
{
	(void)unused;
	return "muted volume capture";
}

static void *capture_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void capture_destroy(void *data)
{
	(void)data;
}

static struct obs_source_info capture_info = {
	.id = CAPTURE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = capture_name,
	.create = capture_create,
	.destroy = capture_destroy,
};

struct seen {
	uint32_t packets;
	bool muted;
};

static void capture_cb(void *param, obs_source_t *source, const struct audio_data *audio, bool muted)
{
	struct seen *seen = param;
	(void)source;
	(void)audio;
	seen->packets++;
	seen->muted = muted;
}

static float silence[FRAMES];

/* A fresh public input, removed so it never joins the mix. */
static obs_source_t *capture_source(void)
{
	char name[64];
	snprintf(name, sizeof(name), "muted volume capture %d", source_count++);
	obs_source_t *source = obs_source_create(CAPTURE_ID, name, NULL, NULL);
	assert_non_null(source);
	obs_source_remove(source);
	return source;
}

/* Sends one packet; returns what source_muted() answered for it and the
 * os_gettime_ns() bracket around it. */
static bool packet_muted(obs_source_t *source, uint64_t *before, uint64_t *after)
{
	struct seen seen = {0};
	obs_source_add_audio_capture_callback(source, capture_cb, &seen);

	struct obs_source_audio audio = {
		.data = {(uint8_t *)silence, (uint8_t *)silence},
		.frames = FRAMES,
		.speakers = SPEAKERS_STEREO,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.samples_per_sec = RATE,
		.timestamp = os_gettime_ns() + 3600 * SEC,
	};
	uint64_t start = os_gettime_ns();
	obs_source_output_audio(source, &audio);
	uint64_t end = os_gettime_ns();

	obs_source_remove_audio_capture_callback(source, capture_cb, &seen);
	assert_int_equal(seen.packets, 1);
	if (before)
		*before = start;
	if (after)
		*after = end;
	return seen.muted;
}

enum key { PTM, PTT };

struct gate {
	bool disabled;
	bool user_muted;
	bool muted;
	bool enabled[2];
	bool user_pressed[2];
	bool pressed[2];
	uint64_t stop[2];
	uint64_t delay[2];
};

static void apply_gate(obs_source_t *source, const struct gate *g)
{
	if (g->disabled)
		obs_source_set_enabled(source, false);
	os_atomic_set_bool(&source->user_muted, g->user_muted);
	source->muted = g->muted;
	source->push_to_mute_enabled = g->enabled[PTM];
	source->push_to_talk_enabled = g->enabled[PTT];
	source->user_push_to_mute_pressed = g->user_pressed[PTM];
	source->user_push_to_talk_pressed = g->user_pressed[PTT];
	source->push_to_mute_pressed = g->pressed[PTM];
	source->push_to_talk_pressed = g->pressed[PTT];
	source->push_to_mute_stop_time = g->stop[PTM];
	source->push_to_talk_stop_time = g->stop[PTT];
	source->push_to_mute_delay = g->delay[PTM];
	source->push_to_talk_delay = g->delay[PTT];
}

static uint64_t stop_time(const obs_source_t *source, enum key key)
{
	return key == PTM ? source->push_to_mute_stop_time : source->push_to_talk_stop_time;
}

/* The stop time a pressed key wrote at a moment in [lo, hi]. */
static bool written_in(uint64_t stop, uint64_t lo, uint64_t hi, uint64_t delay_ms, const char *what)
{
	uint64_t delay = delay_ms * MSEC;
	if (stop < lo + delay || stop > hi + delay) {
		fprintf(stderr, "%s: stop time %llu outside [%llu, %llu] + %llu\n", what, (unsigned long long)stop,
			(unsigned long long)lo, (unsigned long long)hi, (unsigned long long)delay);
		return false;
	}
	return true;
}

struct capture_case {
	const char *what;
	struct gate gate;
	bool muted;
	/* which keys the packet wrote; the others keep gate.stop */
	bool wrote[2];
};

#define FAR (UINT64_MAX)

static const struct capture_case capture_cases[] = {
	{"nothing set", {0}, false, {0}},
	{"disabled", {.disabled = true}, true, {0}},
	{"user muted", {.user_muted = true}, true, {0}},
	/* the mix thread's flag is not the capture's */
	{"mix muted only", {.muted = true}, false, {0}},
	{"disabled and user muted", {.disabled = true, .user_muted = true}, true, {0}},

	/* push-to-mute */
	{"ptm pressed", {.enabled = {true}, .user_pressed = {true}, .delay = {DELAY_MS}}, true, {true}},
	{"ptm pressed, no delay", {.enabled = {true}, .user_pressed = {true}}, true, {true}},
	{"ptm released, held", {.enabled = {true}, .stop = {FAR}}, true, {0}},
	{"ptm released, past", {.enabled = {true}, .stop = {1}}, false, {0}},
	{"ptm released, never", {.enabled = {true}}, false, {0}},
	{"ptm mix pressed only", {.enabled = {true}, .pressed = {true}}, false, {0}},
	{"ptm off, pressed", {.user_pressed = {true}, .delay = {DELAY_MS}}, false, {0}},
	{"ptm off, held", {.stop = {FAR}}, false, {0}},

	/* push-to-talk */
	{"ptt released, never", {.enabled = {false, true}}, true, {0}},
	{"ptt pressed",
	 {.enabled = {false, true}, .user_pressed = {false, true}, .delay = {0, DELAY_MS}},
	 false,
	 {false, true}},
	{"ptt pressed, no delay", {.enabled = {false, true}, .user_pressed = {false, true}}, false, {false, true}},
	{"ptt released, held", {.enabled = {false, true}, .stop = {0, FAR}}, false, {0}},
	{"ptt released, past", {.enabled = {false, true}, .stop = {0, 1}}, true, {0}},
	{"ptt mix pressed only", {.enabled = {false, true}, .pressed = {false, true}}, true, {0}},
	{"ptt off, released", {.stop = {0, 1}}, false, {0}},
	{"ptt off, pressed", {.user_pressed = {false, true}}, false, {0}},

	/* both keys */
	{"both pressed",
	 {.enabled = {true, true}, .user_pressed = {true, true}, .delay = {DELAY_MS, DELAY_MS}},
	 true,
	 {true, true}},
	{"ptt pressed, ptm held",
	 {.enabled = {true, true}, .user_pressed = {false, true}, .stop = {FAR, 0}},
	 true,
	 {false, true}},
	{"user muted, ptt pressed",
	 {.user_muted = true, .enabled = {false, true}, .user_pressed = {false, true}},
	 true,
	 {false, true}},
	/* the writes come before the enabled check */
	{"disabled, both pressed",
	 {.disabled = true, .enabled = {true, true}, .user_pressed = {true, true}, .delay = {DELAY_MS, 0}},
	 true,
	 {true, true}},
};

static bool run_capture_case(const struct capture_case *c)
{
	obs_source_t *source = capture_source();
	apply_gate(source, &c->gate);

	uint64_t before, after;
	bool muted = packet_muted(source, &before, &after);
	bool ok = true;
	if (muted != c->muted) {
		fprintf(stderr, "%s: packet %s, expected %s\n", c->what, muted ? "muted" : "not muted",
			c->muted ? "muted" : "not muted");
		ok = false;
	}
	for (int key = PTM; key <= PTT; key++) {
		uint64_t stop = stop_time(source, key);
		if (c->wrote[key]) {
			ok &= written_in(stop, before, after, c->gate.delay[key], c->what);
		} else if (stop != c->gate.stop[key]) {
			fprintf(stderr, "%s: stop time %d changed to %llu\n", c->what, key, (unsigned long long)stop);
			ok = false;
		}
	}
	obs_source_release(source);
	return ok;
}

static void capture_mute_check(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	bool ok = true;
	for (size_t i = 0; i < sizeof(capture_cases) / sizeof(capture_cases[0]); i++)
		ok &= run_capture_case(&capture_cases[i]);
	if (!ok)
		fail();
}

/* A key released after a real delay: held strictly before its stop time,
 * read against os_gettime_ns() at each packet. */
static void capture_release_inside_and_after_delay(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	for (int key = PTM; key <= PTT; key++) {
		obs_source_t *source = capture_source();
		struct gate g = {.delay = {DELAY_MS, DELAY_MS}};
		g.enabled[key] = true;
		g.user_pressed[key] = true;
		apply_gate(source, &g);

		uint64_t before, after;
		/* pressed: push-to-mute mutes, push-to-talk lets it through */
		assert_int_equal(packet_muted(source, &before, &after), key == PTM);
		assert_true(written_in(stop_time(source, key), before, after, DELAY_MS, "press"));
		uint64_t stop = stop_time(source, key);

		if (key == PTM)
			source->user_push_to_mute_pressed = false;
		else
			source->user_push_to_talk_pressed = false;

		/* inside the delay the key still holds, and nothing is written */
		uint64_t now = os_gettime_ns();
		if (now + 50 * MSEC < stop) {
			assert_int_equal(packet_muted(source, NULL, NULL), key == PTM);
			assert_true(stop_time(source, key) == stop);
		}

		/* after it the key no longer holds */
		while (os_gettime_ns() <= stop)
			os_sleep_ms(10);
		assert_int_equal(packet_muted(source, NULL, NULL), key == PTT);
		assert_true(stop_time(source, key) == stop);
		obs_source_release(source);
	}
}

/* ------------------------------------------------------------------------ */
/* the mix: get_source_volume() */

struct live {
	obs_source_t *source;
	pthread_t thread;
	bool thread_started;
	volatile bool stop;
};

static void *live_thread(void *param)
{
	struct live *c = param;
	float left[PACKET_FRAMES];
	static float right[PACKET_FRAMES];
	for (size_t i = 0; i < PACKET_FRAMES; i++)
		left[i] = LEVEL;

	/* the first packet comes after the case has set the fields */
	os_sleep_ms(50);
	uint64_t t0 = os_gettime_ns();
	for (uint64_t n = 0; !os_atomic_load_bool(&c->stop); n++) {
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

static const char *live_name(void *unused)
{
	(void)unused;
	return "muted volume live";
}

static void *live_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	struct live *c = bzalloc(sizeof(*c));
	c->source = source;
	if (pthread_create(&c->thread, NULL, live_thread, c) == 0)
		c->thread_started = true;
	else
		fprintf(stderr, "could not start the capture thread\n");
	return c;
}

static void live_destroy(void *data)
{
	struct live *c = data;
	if (c->thread_started) {
		os_atomic_set_bool(&c->stop, true);
		pthread_join(c->thread, NULL);
	}
	bfree(c);
}

static struct obs_source_info live_info = {
	.id = LIVE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_AUDIO,
	.get_name = live_name,
	.create = live_create,
	.destroy = live_destroy,
};

struct program {
	pthread_mutex_t mutex;
	size_t matching; /* left at LEVEL * VOLUME, right silent */
	size_t other;    /* anything but that or silence */
	float first_other;
};

static struct program program;

static void raw_audio(void *param, size_t mix_idx, struct audio_data *data)
{
	(void)mix_idx;
	struct program *p = param;
	const float *left = (const float *)data->data[0];
	const float *right = (const float *)data->data[1];

	pthread_mutex_lock(&p->mutex);
	for (uint32_t i = 0; i < data->frames; i++) {
		if (right[i] != 0.0f || (left[i] != 0.0f && fabsf(left[i] - LEVEL * VOLUME) > TOLERANCE)) {
			if (!p->other++)
				p->first_other = right[i] != 0.0f ? right[i] : left[i];
		} else if (left[i] != 0.0f) {
			p->matching++;
		}
	}
	pthread_mutex_unlock(&p->mutex);
}

static void read_program(size_t *matching, size_t *other, float *first_other)
{
	pthread_mutex_lock(&program.mutex);
	*matching = program.matching;
	*other = program.other;
	*first_other = program.first_other;
	pthread_mutex_unlock(&program.mutex);
}

static void reset_program(void)
{
	pthread_mutex_lock(&program.mutex);
	program.matching = 0;
	program.other = 0;
	pthread_mutex_unlock(&program.mutex);
}

/* the source of the current mix case, released even if the case failed */
static obs_source_t *live;

static int case_teardown(void **state)
{
	(void)state;
	if (!live)
		return 0;
	obs_set_output_source(0, NULL);
	obs_source_remove(live);
	obs_source_release(live);
	live = NULL;
	return 0;
}

struct mix_case {
	const char *what;
	struct gate gate;
	/* the program carries the source at VOLUME, or is silent */
	bool carries;
	bool wrote[2];
};

static const struct mix_case mix_cases[] = {
	{"nothing set", {0}, true, {0}},
	{"disabled", {.disabled = true}, false, {0}},
	{"muted", {.muted = true}, false, {0}},
	/* the user's flag is not the mix's */
	{"user muted only", {.user_muted = true}, true, {0}},

	/* push-to-mute */
	{"ptm pressed", {.enabled = {true}, .pressed = {true}, .delay = {DELAY_MS}}, false, {true}},
	{"ptm released, held", {.enabled = {true}, .stop = {FAR}}, false, {0}},
	{"ptm released, past", {.enabled = {true}, .stop = {1}}, true, {0}},
	{"ptm off, pressed", {.pressed = {true}, .delay = {DELAY_MS}}, true, {0}},

	/* push-to-talk */
	{"ptt released, never", {.enabled = {false, true}}, false, {0}},
	{"ptt pressed",
	 {.enabled = {false, true}, .pressed = {false, true}, .delay = {0, DELAY_MS}},
	 true,
	 {false, true}},
	{"ptt released, held", {.enabled = {false, true}, .stop = {0, FAR}}, true, {0}},
	{"ptt off, released", {.stop = {0, 1}}, true, {0}},

	/* both keys, and the writes before the enabled check */
	{"both pressed", {.enabled = {true, true}, .pressed = {true, true}}, false, {true, true}},
	{"disabled, ptt pressed",
	 {.disabled = true, .enabled = {false, true}, .pressed = {false, true}, .delay = {0, DELAY_MS}},
	 false,
	 {false, true}},
};

static bool run_mix_case(const struct mix_case *c)
{
	char name[64];
	snprintf(name, sizeof(name), "muted volume live %d", source_count++);
	live = obs_source_create(LIVE_ID, name, NULL, NULL);
	assert_non_null(live);
	/* before the first packet and before the source is in the mix */
	apply_gate(live, &c->gate);
	live->volume = VOLUME;

	/* what is left of the previous case leaves the mix first */
	os_sleep_ms(100);
	reset_program();

	uint64_t start = os_gettime_ns();
	obs_set_output_source(0, live);

	size_t matching = 0, other = 0;
	float first_other = 0.0f;
	int limit = c->carries ? WAIT_MS : LISTEN_MS;
	for (int waited = 0; waited < limit; waited += 20) {
		os_sleep_ms(20);
		read_program(&matching, &other, &first_other);
		if (other || (c->carries && matching >= WANT_FRAMES))
			break;
	}
	obs_set_output_source(0, NULL);
	/* the mix thread is done with the source */
	os_sleep_ms(100);
	uint64_t end = os_gettime_ns();
	read_program(&matching, &other, &first_other);

	bool ok = true;
	fprintf(stderr, "%s: %zu frames at %g, %zu other\n", c->what, matching, (double)(LEVEL * VOLUME), other);
	if (other) {
		fprintf(stderr, "%s: mixed sample %g, expected %g or silence\n", c->what, (double)first_other,
			(double)(LEVEL * VOLUME));
		ok = false;
	}
	if (c->carries && matching < WANT_FRAMES) {
		fprintf(stderr, "%s: only %zu of %d frames carry the source at its volume\n", c->what, matching,
			WANT_FRAMES);
		ok = false;
	}
	if (!c->carries && matching) {
		fprintf(stderr, "%s: %zu frames carry the source, expected silence\n", c->what, matching);
		ok = false;
	}
	for (int key = PTM; key <= PTT; key++) {
		uint64_t stop = stop_time(live, key);
		if (c->wrote[key]) {
			/* the audio timestamp, the packets' clock, trails the
			 * system clock by the mix's buffering */
			ok &= written_in(stop, start - 2 * SEC, end, c->gate.delay[key], c->what);
		} else if (stop != c->gate.stop[key]) {
			fprintf(stderr, "%s: stop time %d changed to %llu\n", c->what, key, (unsigned long long)stop);
			ok = false;
		}
	}
	case_teardown(NULL);
	return ok;
}

static bool video_up;

static void mix_volume(void **state)
{
	(void)state;
	if (!video_up)
		skip();

	bool ok = true;
	for (size_t i = 0; i < sizeof(mix_cases) / sizeof(mix_cases[0]); i++)
		ok &= run_mix_case(&mix_cases[i]);
	if (!ok)
		fail();
}

/* ------------------------------------------------------------------------ */

static int group_setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);

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
	obs_register_source(&live_info);
	pthread_mutex_init(&program.mutex, NULL);
	obs_add_raw_audio_callback(0, NULL, raw_audio, &program);
	obs_up = true;
	video_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_remove_raw_audio_callback(0, raw_audio, &program);
		obs_shutdown();
		pthread_mutex_destroy(&program.mutex);
		obs_up = false;
		video_up = false;
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(capture_mute_check),
		cmocka_unit_test(capture_release_inside_and_after_delay),
		cmocka_unit_test_teardown(mix_volume, case_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
