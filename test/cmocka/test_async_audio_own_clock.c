/*
 * An async source whose audio is stamped on a clock of its own, apart from
 * its video, must be heard on the program (#3747).
 *
 * NDI Test Patterns stamps its audio about 14191 s ahead of its video, and
 * DistroAV hands both to libobs with those timestamps. libobs places a
 * source's audio by the timing of its video (source->timing_adjust, from the
 * frame being shown). When that placement is more than MAX_TS_VAR (2 s) from
 * where the audio ran, the audio path used to reset its timing to the system
 * clock. Since #522, an audio packet that continued seamlessly follows the
 * video's timing instead, so a Video Delay (Async) filter can delay audio by
 * more than 2 s (#521). For audio on its own clock that puts it hours away
 * from the mix: the program is silent until a packet that does not continue
 * the previous one resets the timing, and the next video frame moves it away
 * again.
 *
 * The test registers such a source: a thread delivers 30 fps frames and
 * packets of a 1 kHz tone, the frames stamped from the moment it started and
 * the audio OFFSET away from them. The source is put on the program, and the
 * mixed audio libobs hands to outputs (obs_add_raw_audio_callback) must carry
 * the tone. Mixing needs core video, so the cases run under xvfb-run and skip
 * without a display.
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

#define SOURCE_ID "test_async_audio_own_clock"

#define W 64
#define H 64
#define RATE 48000
#define SEC 1000000000ULL
#define TONE_HZ 1000.0
#define TONE_LEVEL 0.5f
#define LISTEN_MS 6000
#define SETTLE_MS 1500
/* the audio clock of NDI Test Patterns runs about 14191 s ahead of its video */
#define HOUR (3600ULL * SEC)

const char *obs_module_text(const char *val)
{
	return val;
}

static bool obs_up;

/* ------------------------------------------------------------------------ */
/* the source: async video, and audio on a clock of its own */

struct av {
	obs_source_t *source;
	size_t packet_frames;
	int64_t audio_offset; /* ns, audio timestamps minus video timestamps */
	pthread_t thread;
	bool thread_started;
	volatile bool stop;
};

static void *av_thread(void *param)
{
	struct av *c = param;
	uint8_t *pixels = bmalloc(W * H * 4);
	memset(pixels, 0x80, W * H * 4);
	float *buf = bmalloc(c->packet_frames * sizeof(float));

	const uint64_t packet_ns = util_mul_div64(c->packet_frames, SEC, RATE);
	const uint64_t frame_ns = SEC / 30;
	const uint64_t t0 = os_gettime_ns();
	/* both clocks well away from zero, so an offset in either direction
	 * stays positive */
	const uint64_t base = 4 * HOUR;
	uint64_t packets = 0, frames = 0;

	while (!os_atomic_load_bool(&c->stop)) {
		uint64_t next_packet = t0 + packets * packet_ns;
		uint64_t next_frame = t0 + frames * frame_ns;

		if (next_packet <= next_frame) {
			os_sleepto_ns(next_packet);
			for (size_t i = 0; i < c->packet_frames; i++) {
				uint64_t n = packets * c->packet_frames + i;
				buf[i] = TONE_LEVEL * (float)sin(2.0 * M_PI * TONE_HZ * (double)n / RATE);
			}
			struct obs_source_audio audio = {
				.data = {(const uint8_t *)buf},
				.frames = (uint32_t)c->packet_frames,
				.speakers = SPEAKERS_MONO,
				.format = AUDIO_FORMAT_FLOAT,
				.samples_per_sec = RATE,
				.timestamp = (uint64_t)((int64_t)(base + next_packet - t0) + c->audio_offset),
			};
			obs_source_output_audio(c->source, &audio);
			packets++;
		} else {
			os_sleepto_ns(next_frame);
			struct obs_source_frame frame = {
				.data = {pixels},
				.linesize = {W * 4},
				.width = W,
				.height = H,
				.format = VIDEO_FORMAT_BGRA,
				.timestamp = base + next_frame - t0,
			};
			obs_source_output_video(c->source, &frame);
			frames++;
		}
	}

	bfree(buf);
	bfree(pixels);
	return NULL;
}

static void stop_av(struct av *c)
{
	if (!c->thread_started)
		return;
	os_atomic_set_bool(&c->stop, true);
	pthread_join(c->thread, NULL);
	c->thread_started = false;
}

static const char *av_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "async source with audio on its own clock";
}

static void *av_create(obs_data_t *settings, obs_source_t *source)
{
	struct av *c = bzalloc(sizeof(*c));
	c->source = source;
	c->packet_frames = (size_t)obs_data_get_int(settings, "packet_frames");
	c->audio_offset = obs_data_get_int(settings, "audio_offset");
	return c;
}

static void av_destroy(void *data)
{
	struct av *c = data;
	stop_av(c);
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
	size_t samples;
	size_t loud;
};

static struct program program;

static void raw_audio(void *param, size_t mix_idx, struct audio_data *data)
{
	UNUSED_PARAMETER(mix_idx);
	struct program *p = param;
	const float *left = (const float *)data->data[0];

	pthread_mutex_lock(&p->mutex);
	if (p->listening) {
		for (uint32_t i = 0; i < data->frames; i++) {
			if (fabsf(left[i]) > TONE_LEVEL / 2)
				p->loud++;
			p->samples++;
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
/* running the source on the program */

static obs_source_t *live;

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (!live)
		return 0;
	obs_set_output_source(0, NULL);
	struct av *c = obs_obj_get_data(live);
	if (c && c->source == live)
		stop_av(c);
	obs_source_remove(live);
	obs_source_release(live);
	live = NULL;
	return 0;
}

/* Puts the source on the program, lets it settle, and returns the share of
 * program samples that carry the tone, in percent. */
static size_t heard_percent(size_t packet_frames, int64_t audio_offset)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "packet_frames", (long long)packet_frames);
	obs_data_set_int(settings, "audio_offset", audio_offset);
	live = obs_source_create(SOURCE_ID, "own clock source", settings, NULL);
	obs_data_release(settings);
	struct av *c = live ? obs_obj_get_data(live) : NULL;
	if (!c || c->source != live) {
		fprintf(stderr, "could not create the source\n");
		fail();
	}

	obs_set_output_source(0, live);
	os_atomic_set_bool(&c->stop, false);
	if (pthread_create(&c->thread, NULL, av_thread, c) != 0) {
		fprintf(stderr, "could not start the source thread\n");
		fail();
	}
	c->thread_started = true;
	os_sleep_ms(SETTLE_MS);

	pthread_mutex_lock(&program.mutex);
	program.samples = program.loud = 0;
	program.listening = true;
	pthread_mutex_unlock(&program.mutex);
	os_sleep_ms(LISTEN_MS);
	pthread_mutex_lock(&program.mutex);
	program.listening = false;
	size_t samples = program.samples, loud = program.loud;
	pthread_mutex_unlock(&program.mutex);

	if (samples < (size_t)RATE * (LISTEN_MS / 1000) * 9 / 10) {
		fprintf(stderr, "the program audio did not run (%zu samples)\n", samples);
		fail();
	}
	/* a 1 kHz sine is above half its peak 2/3 of the time */
	size_t percent = loud * 100 * 3 / 2 / samples;
	fprintf(stderr, "%zu-frame packets, audio %+lld s from video: tone in %zu %% of the program\n", packet_frames,
		(long long)(audio_offset / (int64_t)SEC), percent);
	return percent;
}

/* ------------------------------------------------------------------------ */
/* cases */

/* NDI Test Patterns: 100 ms packets, audio hours ahead of the video. */
static void audio_hours_ahead_of_video_is_heard(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	assert_true(heard_percent(4800, (int64_t)(4 * HOUR)) >= 90);
}

static void audio_hours_behind_video_is_heard(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	assert_true(heard_percent(4800, -(int64_t)(3 * HOUR)) >= 90);
}

static void small_packets_audio_ahead_of_video_is_heard(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	assert_true(heard_percent(1024, (int64_t)(4 * HOUR)) >= 90);
}

/* Guard: audio and video on one clock, as a BirdDog camera sends them. */
static void audio_on_the_video_clock_is_heard(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	assert_true(heard_percent(4800, 0) >= 90);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(audio_hours_ahead_of_video_is_heard, case_teardown),
		cmocka_unit_test_teardown(audio_hours_behind_video_is_heard, case_teardown),
		cmocka_unit_test_teardown(small_packets_audio_ahead_of_video_is_heard, case_teardown),
		cmocka_unit_test_teardown(audio_on_the_video_clock_is_heard, case_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
