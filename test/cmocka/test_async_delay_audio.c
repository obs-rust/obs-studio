/*
 * A Video Delay (Async) filter of more than two seconds must not break up the
 * audio of the source it delays (upstream obsproject/obs-studio#13830,
 * reported by simoncos).
 *
 * With a Media Source and the filter set to 4 s or more, the recorded audio
 * turned into "broadband grain": at 2 s and below it was clean. The filter
 * holds the frames back and hands them on with their original timestamps.
 * libobs then places the source's audio by the timing of its video
 * (source->timing_adjust, from the frame being shown), so audio is delayed
 * along with the video. When the video timing moved by more than
 * MAX_TS_VAR (2 s) from one audio packet to the next, the audio path treated
 * it as a timestamp jump and reset its timing to the system clock, the next
 * video frame set it back, and so on: every packet or two was placed by the
 * other timing, cutting the tone up.
 *
 * Sources whose timestamps are not the system clock hit it (the Media Source
 * stamps from the time it was created). The test registers such a source: a
 * thread delivers 30 fps frames and 1024-frame packets of a 1 kHz tone (or of
 * silence with a click every 1.5 s), stamped from the moment it started. The
 * source and its filter are put on the program, and the mixed audio libobs
 * hands to outputs (obs_add_raw_audio_callback) is checked: the tone must be
 * continuous, and a click must come out delayed as much as the video. Mixing
 * needs core video, so the cases run under xvfb-run and skip without a
 * display.
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
#include <util/platform.h>
#include <util/threading.h>
#include <util/util_uint64.h>

#define SOURCE_ID "test_async_delay_av"

#define W 64
#define H 64
#define RATE 48000
#define MSEC 1000000ULL
#define PACKET_FRAMES 1024
#define TONE_HZ 1000.0
#define TONE_LEVEL 0.5f
/* a sample that a 1 kHz sine at TONE_LEVEL cannot reach from the two before
 * it is a break in the tone */
#define BREAK_LEVEL (0.05f * TONE_LEVEL)
#define CLICK_EVERY 70 /* packets: 1.49 s */
/* Clicks are told apart by their length, CLICK_UNIT frames times 1 to
 * CLICK_IDS: a click is matched to the last one of its length captured
 * before it was heard, unambiguous for delays up to 7 s. */
#define CLICK_UNIT 60
#define CLICK_IDS 5
#define CLICK_LEVEL 0.4f
#define QUIET_FRAMES 480
#define MAX_EVENTS 256
#define LISTEN_MS 4000
#define SETTLE_MS 1500
/* The delay comes out a few video frames and mixer ticks long (1080 ms for
 * 1 s here); the broken case is off by the whole delay. */
#define TOLERANCE_MS 200

extern struct obs_source_info async_delay_filter;

const char *obs_module_text(const char *val)
{
	return val;
}

static bool obs_up;

/* ------------------------------------------------------------------------ */
/* the source: async video and audio on a clock of its own */

struct av {
	obs_source_t *source;
	bool clicks;
	pthread_t thread;
	bool thread_started;
	volatile bool stop;

	pthread_mutex_t mutex;
	uint64_t click_sys_ts[MAX_EVENTS];
	int click_ids[MAX_EVENTS];
	size_t num_clicks;
};

static struct av *current;

static void *av_thread(void *param)
{
	struct av *c = param;
	uint8_t *pixels = bmalloc(W * H * 4);
	memset(pixels, 0x80, W * H * 4);
	float buf[PACKET_FRAMES];

	const uint64_t packet_ns = util_mul_div64(PACKET_FRAMES, 1000000000ULL, RATE);
	const uint64_t frame_ns = 1000000000ULL / 30;
	const uint64_t t0 = os_gettime_ns();
	uint64_t packets = 0, frames = 0;

	while (!c->stop) {
		uint64_t next_packet = t0 + packets * packet_ns;
		uint64_t next_frame = t0 + frames * frame_ns;

		if (next_packet <= next_frame) {
			bool is_click = c->clicks && packets % CLICK_EVERY == CLICK_EVERY / 2;
			int click_id = (int)((packets / CLICK_EVERY) % CLICK_IDS);
			size_t click_frames = (size_t)CLICK_UNIT * (size_t)(click_id + 1);
			os_sleepto_ns(next_packet);

			for (size_t i = 0; i < PACKET_FRAMES; i++) {
				if (c->clicks) {
					buf[i] = is_click && i < click_frames ? ((i & 4) ? 0.8f : -0.8f) : 0.0f;
				} else {
					uint64_t n = packets * PACKET_FRAMES + i;
					buf[i] = TONE_LEVEL * (float)sin(2.0 * M_PI * TONE_HZ * (double)n / RATE);
				}
			}

			/* stamped from when the source started, like a Media
			 * Source, not with the system clock */
			struct obs_source_audio audio = {
				.data = {(const uint8_t *)buf},
				.frames = PACKET_FRAMES,
				.speakers = SPEAKERS_MONO,
				.format = AUDIO_FORMAT_FLOAT,
				.samples_per_sec = RATE,
				.timestamp = next_packet - t0,
			};
			obs_source_output_audio(c->source, &audio);

			if (is_click) {
				pthread_mutex_lock(&c->mutex);
				if (c->num_clicks < MAX_EVENTS) {
					c->click_ids[c->num_clicks] = click_id;
					c->click_sys_ts[c->num_clicks++] = next_packet;
				}
				pthread_mutex_unlock(&c->mutex);
			}
			packets++;
		} else {
			os_sleepto_ns(next_frame);
			struct obs_source_frame frame = {
				.data = {pixels},
				.linesize = {W * 4},
				.width = W,
				.height = H,
				.format = VIDEO_FORMAT_BGRA,
				.timestamp = next_frame - t0,
			};
			obs_source_output_video(c->source, &frame);
			frames++;
		}
	}

	bfree(pixels);
	return NULL;
}

static void start_av(struct av *c)
{
	c->stop = false;
	if (pthread_create(&c->thread, NULL, av_thread, c) != 0) {
		fprintf(stderr, "could not start the source thread\n");
		fail();
	}
	c->thread_started = true;
}

static void stop_av(struct av *c)
{
	if (!c->thread_started)
		return;
	c->stop = true;
	pthread_join(c->thread, NULL);
	c->thread_started = false;
}

static const char *av_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "async audio/video source";
}

static void *av_create(obs_data_t *settings, obs_source_t *source)
{
	struct av *c = bzalloc(sizeof(*c));
	c->source = source;
	c->clicks = obs_data_get_bool(settings, "clicks");
	pthread_mutex_init(&c->mutex, NULL);
	current = c;
	return c;
}

static void av_destroy(void *data)
{
	struct av *c = data;
	stop_av(c);
	pthread_mutex_destroy(&c->mutex);
	if (current == c)
		current = NULL;
	bfree(c);
}

static uint32_t av_width(void *data)
{
	UNUSED_PARAMETER(data);
	return W;
}

static uint32_t av_height(void *data)
{
	UNUSED_PARAMETER(data);
	return H;
}

static struct obs_source_info av_info = {
	.id = SOURCE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_ASYNC_VIDEO | OBS_SOURCE_AUDIO,
	.get_name = av_name,
	.create = av_create,
	.destroy = av_destroy,
	.get_width = av_width,
	.get_height = av_height,
};

/* ------------------------------------------------------------------------ */
/* the program audio */

struct program {
	pthread_mutex_t mutex;
	bool listening;
	float prev[2];
	size_t seen;
	size_t samples;
	size_t loud;
	size_t breaks;
	size_t since_break;
	uint64_t clicks[MAX_EVENTS];
	int click_ids[MAX_EVENTS];
	size_t num_clicks;
	size_t quiet;
	size_t click_len;
};

static struct program program;

static void raw_audio(void *param, size_t mix_idx, struct audio_data *data)
{
	UNUSED_PARAMETER(mix_idx);
	struct program *p = param;
	const float *left = (const float *)data->data[0];
	const float c = 2.0f * (float)cos(2.0 * M_PI * TONE_HZ / RATE);

	pthread_mutex_lock(&p->mutex);
	for (uint32_t i = 0; i < data->frames; i++) {
		float y = left[i];

		if (p->listening && p->seen >= 2) {
			/* y[n] + y[n-2] == 2 cos(w) y[n-1] for a sine of
			 * frequency w; a break counts once */
			float r = y + p->prev[0] - c * p->prev[1];
			if (fabsf(r) > BREAK_LEVEL) {
				if (p->since_break > 64)
					p->breaks++;
				p->since_break = 0;
			} else {
				p->since_break++;
			}
			if (fabsf(y) > TONE_LEVEL / 2)
				p->loud++;
			p->samples++;
		}
		p->prev[0] = p->prev[1];
		p->prev[1] = y;
		p->seen++;

		if (y > CLICK_LEVEL || y < -CLICK_LEVEL) {
			if (p->quiet >= QUIET_FRAMES && p->num_clicks < MAX_EVENTS) {
				p->clicks[p->num_clicks] = data->timestamp + util_mul_div64(i, 1000000000ULL, RATE);
				p->click_ids[p->num_clicks++] = -1;
				p->click_len = 0;
			}
			p->click_len++;
			p->quiet = 0;
		} else {
			/* the click ended: its length tells which one it was */
			if (p->quiet == 0 && p->num_clicks && p->click_ids[p->num_clicks - 1] < 0)
				p->click_ids[p->num_clicks - 1] =
					(int)((p->click_len + CLICK_UNIT / 2) / CLICK_UNIT) - 1;
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

	obs_register_source(&av_info);
	obs_register_source(&async_delay_filter);
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
/* running a source through the filter */

/* the source and filter under test, cleaned up after each case even if it
 * failed */
static obs_source_t *live;
static obs_source_t *live_filter;

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!live)
		return 0;
	obs_set_output_source(0, NULL);
	if (current && current->source == live)
		stop_av(current);
	if (live_filter) {
		obs_source_filter_remove(live, live_filter);
		obs_source_release(live_filter);
		live_filter = NULL;
	}
	obs_source_remove(live);
	obs_source_release(live);
	live = NULL;
	return 0;
}

/* Puts a source on the program behind a Video Delay (Async) filter of
 * @delay_ms (none for a negative value) and waits until the delayed audio
 * plays. The audio plays undelayed until the first delayed frame is shown
 * (after @delay_ms); from then on the audio follows the video, so what was
 * captured then is heard @delay_ms later, and the program is silent in
 * between. */
static struct av *start_delayed(bool clicks, int delay_ms)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_bool(settings, "clicks", clicks);
	live = obs_source_create(SOURCE_ID, "delayed source", settings, NULL);
	obs_data_release(settings);
	if (!live || !current || current->source != live) {
		fprintf(stderr, "could not create the source\n");
		fail();
	}
	struct av *c = current;

	if (delay_ms >= 0) {
		obs_data_t *fs = obs_data_create();
		obs_data_set_int(fs, "delay_ms", delay_ms);
		live_filter = obs_source_create_private("async_delay_filter", "Video Delay", fs);
		obs_data_release(fs);
		obs_source_filter_add(live, live_filter);
	}

	obs_set_output_source(0, live);
	start_av(c);
	os_sleep_ms(2 * (uint32_t)(delay_ms > 0 ? delay_ms : 0) + SETTLE_MS);
	return c;
}

static void listen(void)
{
	pthread_mutex_lock(&program.mutex);
	program.samples = program.loud = program.breaks = 0;
	program.since_break = 0;
	program.num_clicks = 0;
	program.listening = true;
	pthread_mutex_unlock(&program.mutex);

	os_sleep_ms(LISTEN_MS);

	pthread_mutex_lock(&program.mutex);
	program.listening = false;
	pthread_mutex_unlock(&program.mutex);
}

static void expect_continuous_tone(int delay_ms)
{
	start_delayed(false, delay_ms);
	listen();

	pthread_mutex_lock(&program.mutex);
	size_t samples = program.samples, loud = program.loud, breaks = program.breaks;
	pthread_mutex_unlock(&program.mutex);

	fprintf(stderr, "video delay %d ms: %zu samples heard, %zu loud, tone broken %zu times\n", delay_ms, samples,
		loud, breaks);

	if (samples < (size_t)RATE * (LISTEN_MS / 1000) * 9 / 10) {
		fprintf(stderr, "video delay %d ms: the program audio did not run\n", delay_ms);
		fail();
	}
	if (loud < samples / 2) {
		fprintf(stderr, "video delay %d ms: the tone did not reach the program\n", delay_ms);
		fail();
	}
	if (breaks != 0) {
		fprintf(stderr, "video delay %d ms: the tone was broken up %zu times in %d ms\n", delay_ms, breaks,
			LISTEN_MS);
		fail();
	}
}

/* Each click heard on the program must lag its capture by @delay_ms: the
 * audio stays in sync with the delayed video. */
static void expect_audio_delay(int delay_ms)
{
	struct av *c = start_delayed(true, delay_ms);
	listen();

	int64_t delays[MAX_EVENTS];
	size_t found = 0;

	pthread_mutex_lock(&c->mutex);
	pthread_mutex_lock(&program.mutex);
	for (size_t i = 0; i < program.num_clicks; i++) {
		uint64_t heard = program.clicks[i];
		uint64_t captured = 0;

		/* the latest click of its length captured before it was
		 * heard */
		for (size_t j = 0; j < c->num_clicks; j++) {
			if (c->click_ids[j] == program.click_ids[i] && c->click_sys_ts[j] <= heard + 5 * MSEC)
				captured = c->click_sys_ts[j];
		}
		if (captured)
			delays[found++] = ((int64_t)heard - (int64_t)captured) / (int64_t)MSEC;
	}
	pthread_mutex_unlock(&program.mutex);
	pthread_mutex_unlock(&c->mutex);

	fprintf(stderr, "video delay %d ms: audio delayed by", delay_ms);
	for (size_t i = 0; i < found; i++)
		fprintf(stderr, " %lld", (long long)delays[i]);
	fprintf(stderr, " ms\n");

	if (found < 2) {
		fprintf(stderr, "video delay %d ms: only %zu clicks reached the program\n", delay_ms, found);
		fail();
	}
	for (size_t i = 0; i < found; i++) {
		int64_t want = delay_ms > 0 ? delay_ms : 0;
		if (delays[i] < want - TOLERANCE_MS || delays[i] > want + TOLERANCE_MS) {
			fprintf(stderr, "video delay %d ms: audio delayed by %lld ms, not with the video\n", delay_ms,
				(long long)delays[i]);
			fail();
		}
	}
}

/* ------------------------------------------------------------------------ */
/* cases */

/* The report: 4 s and more broke the audio up. */
static void tone_continuous_with_3s_video_delay(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	expect_continuous_tone(3000);
}

static void tone_continuous_with_5s_video_delay(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	expect_continuous_tone(5000);
}

/* The audio is delayed along with the video, as it already was below 2 s. */
static void audio_follows_3s_video_delay(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	expect_audio_delay(3000);
}

/* Guards: below 2 s, and without the filter, this worked already. */
static void tone_continuous_with_1s_video_delay(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	expect_continuous_tone(1000);
}

static void audio_follows_1s_video_delay(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	expect_audio_delay(1000);
}

static void audio_not_delayed_without_filter(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	expect_audio_delay(-1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(tone_continuous_with_3s_video_delay, case_teardown),
		cmocka_unit_test_teardown(tone_continuous_with_5s_video_delay, case_teardown),
		cmocka_unit_test_teardown(audio_follows_3s_video_delay, case_teardown),
		cmocka_unit_test_teardown(tone_continuous_with_1s_video_delay, case_teardown),
		cmocka_unit_test_teardown(audio_follows_1s_video_delay, case_teardown),
		cmocka_unit_test_teardown(audio_not_delayed_without_filter, case_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
