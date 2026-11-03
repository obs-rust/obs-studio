/*
 * An audio sync offset must apply to a source that is already producing
 * audio (upstream obsproject/obs-studio#7912, reported by Daniel-Noethen;
 * also #11656 by jetpackpaul with davidtriggs, #11873 by ndfan77 and #5356
 * by MasterZap).
 *
 * Changing a running source's sync offset from 0 to anything else did
 * nothing: the source's audio kept playing at the old delay. A change from
 * one non-zero value to another worked, which is why setting it twice
 * "fixed" it. Loading a scene collection hits the same case: a live capture
 * (Pulse/PipeWire, macOS Audio Capture, NDI) starts producing audio when it
 * is created, and the saved offset is set only after that, so it was
 * ignored until it was changed by hand.
 *
 * libobs appends each packet that continues the previous one seamlessly to
 * the source's buffer, and adds the sync offset only to a packet's own
 * timestamp, which an appended packet ignores. A change of the offset must
 * therefore place the next packet by its timestamp, and it did, except when
 * the previous offset was 0.
 *
 * The test registers an audio source that behaves like a live capture: a
 * thread delivers 10 ms packets stamped with the system clock, silent except
 * for a short click every 1.5 s. The source is put on the program, and the
 * clicks are found in the mixed audio libobs hands to outputs
 * (obs_add_raw_audio_callback). How far each click's timestamp there lags
 * the timestamp it was captured with is the delay the offset must produce.
 * Mixing needs core video, so the cases run under xvfb-run and skip without
 * a display.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/platform.h>
#include <util/threading.h>
#include <util/util_uint64.h>

#define SOURCE_ID "test_sync_offset_capture"

#define W 64
#define H 64
#define RATE 48000
#define MSEC 1000000LL
#define PACKET_FRAMES 480 /* 10 ms */
#define PACKET_NS (10 * MSEC)
/* Longer than the largest offset tested, so a click is never matched to the
 * one before it. */
#define CLICK_EVERY 150 /* packets: 1.5 s */
#define CLICK_FRAMES 240
#define CLICK_LEVEL 0.4f
#define QUIET_FRAMES 480
#define MAX_EVENTS 256
/* The click is placed by sample, so the delay is exact to a few samples,
 * except that the first source of the process measures one 1024-frame
 * mixer tick (21 ms) at offset 0 until a packet is placed by its timestamp.
 * The offsets tested are 300 ms apart. */
#define TOLERANCE_MS 60
#define WAIT_MS 8000

static bool obs_up;

/* ------------------------------------------------------------------------ */
/* the capture */

struct capture {
	obs_source_t *source;
	pthread_t thread;
	bool thread_started;
	volatile bool stop;
	os_event_t *first_packet;

	pthread_mutex_t mutex;
	uint64_t clicks[MAX_EVENTS];
	size_t num_clicks;
};

static void *capture_thread(void *param)
{
	struct capture *c = param;
	static float silence[PACKET_FRAMES];
	float click[PACKET_FRAMES] = {0};
	for (size_t i = 0; i < CLICK_FRAMES; i++)
		click[i] = (i & 4) ? 0.8f : -0.8f;

	uint64_t t0 = os_gettime_ns();
	for (uint64_t n = 0; !os_atomic_load_bool(&c->stop); n++) {
		uint64_t ts = t0 + n * PACKET_NS;
		bool is_click = n % CLICK_EVERY == CLICK_EVERY / 2;

		/* a period is delivered when it has been captured */
		os_sleepto_ns(ts + PACKET_NS);

		struct obs_source_audio audio = {
			.data = {(const uint8_t *)(is_click ? click : silence)},
			.frames = PACKET_FRAMES,
			.speakers = SPEAKERS_MONO,
			.format = AUDIO_FORMAT_FLOAT,
			.samples_per_sec = RATE,
			.timestamp = ts,
		};
		obs_source_output_audio(c->source, &audio);

		if (is_click) {
			pthread_mutex_lock(&c->mutex);
			if (c->num_clicks < MAX_EVENTS)
				c->clicks[c->num_clicks++] = ts;
			pthread_mutex_unlock(&c->mutex);
		}
		if (n == 0)
			os_event_signal(c->first_packet);
	}
	return NULL;
}

static void start_capture(struct capture *c)
{
	if (c->thread_started)
		return;
	os_atomic_set_bool(&c->stop, false);
	if (pthread_create(&c->thread, NULL, capture_thread, c) != 0) {
		fprintf(stderr, "could not start the capture thread\n");
		return;
	}
	c->thread_started = true;
	os_event_wait(c->first_packet);
}

static void stop_capture(struct capture *c)
{
	if (!c->thread_started)
		return;
	os_atomic_set_bool(&c->stop, true);
	pthread_join(c->thread, NULL);
	c->thread_started = false;
}

static const char *capture_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "live audio capture";
}

/* Like a live capture, it starts capturing when it is created, unless the
 * test wants to set it up first. */
static void *capture_create(obs_data_t *settings, obs_source_t *source)
{
	struct capture *c = bzalloc(sizeof(*c));
	c->source = source;
	pthread_mutex_init(&c->mutex, NULL);
	os_event_init(&c->first_packet, OS_EVENT_TYPE_MANUAL);

	if (!obs_data_get_bool(settings, "deferred"))
		start_capture(c);
	return c;
}

static void capture_destroy(void *data)
{
	struct capture *c = data;
	stop_capture(c);
	os_event_destroy(c->first_packet);
	pthread_mutex_destroy(&c->mutex);
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
/* the program audio */

struct program {
	pthread_mutex_t mutex;
	uint64_t clicks[MAX_EVENTS];
	size_t num_clicks;
	size_t quiet;
};

static struct program program;

static void raw_audio(void *param, size_t mix_idx, struct audio_data *data)
{
	UNUSED_PARAMETER(mix_idx);
	struct program *p = param;
	const float *left = (const float *)data->data[0];

	pthread_mutex_lock(&p->mutex);
	for (uint32_t i = 0; i < data->frames; i++) {
		if (left[i] > CLICK_LEVEL || left[i] < -CLICK_LEVEL) {
			if (p->quiet >= QUIET_FRAMES && p->num_clicks < MAX_EVENTS)
				p->clicks[p->num_clicks++] = data->timestamp + util_mul_div64(i, 1000000000ULL, RATE);
			p->quiet = 0;
		} else {
			p->quiet++;
		}
	}
	pthread_mutex_unlock(&p->mutex);
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
	pthread_mutex_init(&program.mutex, NULL);
	obs_add_raw_audio_callback(0, NULL, raw_audio, &program);

	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up) {
		obs_remove_raw_audio_callback(0, raw_audio, &program);
		obs_shutdown();
		pthread_mutex_destroy(&program.mutex);
		obs_up = false;
	}
	return 0;
}

/* ------------------------------------------------------------------------ */
/* measuring */

/* the source under test, cleaned up after each case even if it failed */
static obs_source_t *live;

static struct capture *capture_of(obs_source_t *source)
{
	live = source;
	struct capture *c = source ? obs_obj_get_data(source) : NULL;
	if (!c || c->source != source) {
		fprintf(stderr, "could not create the capture source\n");
		fail();
	}
	return c;
}

static void put_on_program(obs_source_t *source)
{
	pthread_mutex_lock(&program.mutex);
	program.num_clicks = 0;
	program.quiet = 0;
	pthread_mutex_unlock(&program.mutex);
	obs_set_output_source(0, source);
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!live)
		return 0;
	obs_set_output_source(0, NULL);
	struct capture *c = obs_obj_get_data(live);
	if (c && c->source == live)
		stop_capture(c);
	obs_source_remove(live);
	obs_source_release(live);
	live = NULL;
	return 0;
}

/* Waits for the next @count clicks captured at or after @since to reach the
 * program and checks that each lags its capture by @offset_ms. */
static void expect_delay(struct capture *c, const char *what, uint64_t since, int64_t offset_ms, size_t count)
{
	int64_t delays[8];
	size_t found = 0;

	for (int waited = 0; waited < WAIT_MS && found < count; waited += 20) {
		os_sleep_ms(20);
		found = 0;

		pthread_mutex_lock(&c->mutex);
		pthread_mutex_lock(&program.mutex);
		for (size_t i = 0; i < program.num_clicks && found < count; i++) {
			uint64_t heard = program.clicks[i];
			uint64_t captured = 0;

			/* the latest click captured before it was heard */
			for (size_t j = 0; j < c->num_clicks; j++) {
				if (c->clicks[j] <= heard + 5 * MSEC)
					captured = c->clicks[j];
			}
			if (captured && captured >= since)
				delays[found++] = ((int64_t)heard - (int64_t)captured) / MSEC;
		}
		pthread_mutex_unlock(&program.mutex);
		pthread_mutex_unlock(&c->mutex);
	}

	fprintf(stderr, "%s: sync offset %lld ms, audio delayed by", what, (long long)offset_ms);
	for (size_t i = 0; i < found; i++)
		fprintf(stderr, " %lld", (long long)delays[i]);
	fprintf(stderr, " ms\n");

	if (found < count) {
		fprintf(stderr, "%s: only %zu of %zu clicks reached the program\n", what, found, count);
		fail();
	}
	for (size_t i = 0; i < found; i++) {
		if (delays[i] < offset_ms - TOLERANCE_MS || delays[i] > offset_ms + TOLERANCE_MS) {
			fprintf(stderr,
				"%s: the sync offset was not applied (audio delayed by %lld ms instead of %lld ms)\n",
				what, (long long)delays[i], (long long)offset_ms);
			fail();
		}
	}
}

static obs_source_t *create_capture(const char *name, bool deferred)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "deferred", deferred);
	obs_source_t *source = obs_source_create(SOURCE_ID, name, settings, NULL);
	obs_data_release(settings);
	return source;
}

static void set_offset(obs_source_t *source, int64_t offset_ms, uint64_t *since)
{
	obs_source_set_sync_offset(source, offset_ms * MSEC);
	*since = os_gettime_ns();
}

/* ------------------------------------------------------------------------ */
/* cases */

/* The report: the offset is changed in Advanced Audio Properties (or by a
 * script) while the source is on the program, from its default of 0. */
static void offset_from_zero_applies_to_running_source(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	obs_source_t *source = create_capture("running capture", false);
	struct capture *c = capture_of(source);
	put_on_program(source);

	uint64_t since = os_gettime_ns();
	expect_delay(c, "running, no offset", since, 0, 1);

	set_offset(source, 300, &since);
	expect_delay(c, "running, offset 0 -> 300 ms", since, 300, 2);
}

/* A scene collection is loaded: obs_load_source() creates the capture, which
 * starts producing audio at once, and only then sets the saved offset. The
 * source is shown afterwards. */
static void saved_offset_applies_to_live_capture_on_load(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "id", SOURCE_ID);
	obs_data_set_string(data, "name", "loaded capture");
	obs_data_set_int(data, "sync", 300 * MSEC);
	uint64_t since = os_gettime_ns();
	obs_source_t *source = obs_load_source(data);
	obs_data_release(data);

	struct capture *c = capture_of(source);
	if (obs_source_get_sync_offset(source) != 300 * MSEC) {
		fprintf(stderr, "the saved sync offset was not loaded\n");
		fail();
	}
	put_on_program(source);
	expect_delay(c, "loaded while capturing, then shown", since, 300, 2);
}

/* The offset is set after the source has been shown, while it is capturing:
 * a plugin or script setting it after creating its source. */
static void offset_set_after_activation_applies(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	obs_source_t *source = create_capture("activated capture", false);
	struct capture *c = capture_of(source);
	put_on_program(source);

	uint64_t since;
	set_offset(source, 300, &since);
	expect_delay(c, "shown, then offset set", since, 300, 2);
}

/* Guard: an offset set before the source produces any audio (and before it
 * is shown) applied already. */
static void offset_set_before_audio_applies(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	obs_source_t *source = create_capture("deferred capture", true);
	struct capture *c = capture_of(source);

	uint64_t since;
	set_offset(source, 300, &since);
	put_on_program(source);
	start_capture(c);
	expect_delay(c, "offset set before any audio", since, 300, 2);
}

/* Guard: changes between offsets other than 0 to begin with applied
 * already, back to 0 included. */
static void offset_between_nonzero_values_applies(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();

	obs_source_t *source = create_capture("nonzero capture", true);
	struct capture *c = capture_of(source);

	uint64_t since;
	set_offset(source, 300, &since);
	put_on_program(source);
	start_capture(c);
	expect_delay(c, "offset 300 ms", since, 300, 1);

	set_offset(source, 600, &since);
	expect_delay(c, "offset 300 -> 600 ms", since, 600, 2);

	set_offset(source, 0, &since);
	expect_delay(c, "offset 600 -> 0 ms", since, 0, 2);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(offset_from_zero_applies_to_running_source, case_teardown),
		cmocka_unit_test_teardown(saved_offset_applies_to_live_capture_on_load, case_teardown),
		cmocka_unit_test_teardown(offset_set_after_activation_applies, case_teardown),
		cmocka_unit_test_teardown(offset_set_before_audio_applies, case_teardown),
		cmocka_unit_test_teardown(offset_between_nonzero_values_applies, case_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
