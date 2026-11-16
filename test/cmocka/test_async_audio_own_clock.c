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
 *
 * A sender's audio clock is a crystal of its own: it runs a little faster or
 * slower than the system clock (#4143). Audio on a clock of its own must
 * still come out about when it arrives, not later and later. The drift case
 * sends packets RATE_FAST times as fast as real time, stamped by their
 * samples, makes some packets silent as markers, and measures how long after
 * its packet was sent each marker starts in the program.
 *
 * libobs turns clock drift compensation on for audio on a clock of its own
 * (#4182), so the resampler takes the drift out before the buffer grows past
 * the step of #4143. The resampling cases send RATE_COMPENSATED times as fast
 * as real time and count, through an audio capture callback, the packets that
 * come out of the source's resampler with another length.
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
/* a fast sender clock: 10 % compresses hours of a 50 ppm drift into seconds */
#define RATE_FAST 1.10
#define DRIFT_LISTEN_MS 10000
/* a sender 1 % fast: 10 ms more audio a second, out of the dead band of the
 * resampler's loop within a few seconds */
#define RATE_COMPENSATED 1.01
#define RESAMPLE_LISTEN_MS 5000
/* a marker: one silent packet in MARKER_EVERY, from packet MARKER_FIRST (the
 * program carries the tone well before it) */
#define MARKER_EVERY 10
#define MARKER_FIRST 25
#define MIN_MARKERS 5
#define MAX_MARKERS 64
/* a silence this long in the program is a marker */
#define MARKER_MIN_FRAMES (RATE * 80 / 1000)
#define MAX_MARKER_DELAY (200 * 1000000LL)

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
	double rate;          /* the sender's audio clock over the system clock */
	bool markers;
	uint64_t sent[MAX_MARKERS]; /* when each marker packet was sent */
	size_t num_sent;
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
		/* the packets are stamped by their samples, on the sender's clock */
		uint64_t next_packet = t0 + (uint64_t)((double)(packets * packet_ns) / c->rate);
		bool marker = c->markers && packets >= MARKER_FIRST && (packets - MARKER_FIRST) % MARKER_EVERY == 0;
		uint64_t next_frame = t0 + frames * frame_ns;

		if (next_packet <= next_frame) {
			os_sleepto_ns(next_packet);
			for (size_t i = 0; i < c->packet_frames; i++) {
				uint64_t n = packets * c->packet_frames + i;
				buf[i] = marker ? 0.0f
						: TONE_LEVEL * (float)sin(2.0 * M_PI * TONE_HZ * (double)n / RATE);
			}
			struct obs_source_audio audio = {
				.data = {(const uint8_t *)buf},
				.frames = (uint32_t)c->packet_frames,
				.speakers = SPEAKERS_MONO,
				.format = AUDIO_FORMAT_FLOAT,
				.samples_per_sec = RATE,
				.timestamp = (uint64_t)((int64_t)(base + packets * packet_ns) + c->audio_offset),
			};
			if (marker && c->num_sent < MAX_MARKERS)
				c->sent[c->num_sent++] = os_gettime_ns();
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
	c->rate = obs_data_get_double(settings, "rate");
	if (c->rate <= 0.0)
		c->rate = 1.0;
	c->markers = obs_data_get_bool(settings, "markers");
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
	/* marker starts: the time of the first sample of each silence of at
	 * least MARKER_MIN_FRAMES after the tone was first heard */
	bool tone_heard;
	size_t quiet_frames;
	uint64_t quiet_start;
	uint64_t marker_starts[MAX_MARKERS];
	size_t num_markers;
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
	for (uint32_t i = 0; i < data->frames; i++) {
		if (fabsf(left[i]) >= TONE_LEVEL / 100) {
			if (fabsf(left[i]) > TONE_LEVEL / 2)
				p->tone_heard = true;
			p->quiet_frames = 0;
		} else if (p->tone_heard) {
			if (!p->quiet_frames)
				p->quiet_start = data->timestamp + util_mul_div64(i, SEC, RATE);
			if (++p->quiet_frames == MARKER_MIN_FRAMES && p->num_markers < MAX_MARKERS)
				p->marker_starts[p->num_markers++] = p->quiet_start;
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

/* Puts a source on the program whose audio clock runs `rate` times as fast as
 * the system clock, with markers, and returns the largest distance between
 * a marker's start in the program and the time its packet was sent, in ns. */
static int64_t largest_marker_delay(double rate)
{
	pthread_mutex_lock(&program.mutex);
	program.tone_heard = false;
	program.quiet_frames = 0;
	program.num_markers = 0;
	pthread_mutex_unlock(&program.mutex);

	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "packet_frames", 4800);
	obs_data_set_int(settings, "audio_offset", (int64_t)(4 * HOUR));
	obs_data_set_double(settings, "rate", rate);
	obs_data_set_bool(settings, "markers", true);
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
	os_sleep_ms(DRIFT_LISTEN_MS);
	/* program audio comes about a buffer behind: let the last marker in */
	os_sleep_ms(SETTLE_MS);
	/* the markers heard so far: the silence after the source stops is none */
	pthread_mutex_lock(&program.mutex);
	size_t heard = program.num_markers;
	pthread_mutex_unlock(&program.mutex);
	stop_av(c);

	pthread_mutex_lock(&program.mutex);
	size_t markers = heard < c->num_sent ? heard : c->num_sent;
	int64_t largest = 0;
	fprintf(stderr, "sender clock x%.2f, marker delays (ms):", rate);
	for (size_t k = 0; k < markers; k++) {
		int64_t delay = (int64_t)(program.marker_starts[k] - c->sent[k]);
		fprintf(stderr, " %lld", (long long)(delay / 1000000));
		if (llabs((long long)delay) > llabs((long long)largest))
			largest = delay;
	}
	fprintf(stderr, " (%zu sent, %zu heard)\n", c->num_sent, heard);
	pthread_mutex_unlock(&program.mutex);

	if (markers < MIN_MARKERS) {
		fprintf(stderr, "too few markers came through\n");
		fail();
	}
	return largest;
}

/* The packets the source gave the program, and how many of them the
 * resampler made longer or shorter than they came. */
struct resampled {
	pthread_mutex_t mutex;
	size_t packets;
	size_t changed;
	uint32_t frames;
};

static void count_resampled(void *param, obs_source_t *source, const struct audio_data *audio, bool muted)
{
	UNUSED_PARAMETER(source);
	UNUSED_PARAMETER(muted);
	struct resampled *r = param;

	pthread_mutex_lock(&r->mutex);
	r->packets++;
	if (audio->frames != r->frames)
		r->changed++;
	pthread_mutex_unlock(&r->mutex);
}

/* Puts a source on the program whose audio clock runs `rate` times as fast as
 * the system clock, `audio_offset` from its video, for RESAMPLE_LISTEN_MS, and
 * returns how many of its 100 ms packets came out of its resampler with
 * another length (#4182). */
static size_t resampled_packets(double rate, int64_t audio_offset)
{
	struct resampled r = {.frames = 4800};
	pthread_mutex_init(&r.mutex, NULL);

	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "packet_frames", r.frames);
	obs_data_set_int(settings, "audio_offset", audio_offset);
	obs_data_set_double(settings, "rate", rate);
	live = obs_source_create(SOURCE_ID, "own clock source", settings, NULL);
	obs_data_release(settings);
	struct av *c = live ? obs_obj_get_data(live) : NULL;
	if (!c || c->source != live) {
		fprintf(stderr, "could not create the source\n");
		fail();
	}
	obs_source_add_audio_capture_callback(live, count_resampled, &r);

	obs_set_output_source(0, live);
	os_atomic_set_bool(&c->stop, false);
	if (pthread_create(&c->thread, NULL, av_thread, c) != 0) {
		fprintf(stderr, "could not start the source thread\n");
		fail();
	}
	c->thread_started = true;
	os_sleep_ms(RESAMPLE_LISTEN_MS);
	stop_av(c);
	obs_source_remove_audio_capture_callback(live, count_resampled, &r);

	pthread_mutex_lock(&r.mutex);
	size_t packets = r.packets, changed = r.changed;
	pthread_mutex_unlock(&r.mutex);
	pthread_mutex_destroy(&r.mutex);

	fprintf(stderr, "sender clock x%.2f, audio %+lld s from video: %zu of %zu packets resampled\n", rate,
		(long long)(audio_offset / (int64_t)SEC), changed, packets);
	if (packets < RESAMPLE_LISTEN_MS / 100 / 2) {
		fprintf(stderr, "too few packets came through\n");
		fail();
	}
	return changed;
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

/* #4143: a sender whose audio clock runs fast. Its audio must not come out
 * later and later; before the fix the delay grew by 100 ms every second. */
static void fast_sender_clock_does_not_build_up_delay(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	assert_true(llabs((long long)largest_marker_delay(RATE_FAST)) < MAX_MARKER_DELAY);
}

/* #4182: libobs turns clock drift compensation on for audio on a clock of
 * its own. A sender 1 % fast fills the buffer 10 ms a second faster than the
 * program plays it; within a few seconds the resampler takes samples out, so
 * some packets come out shorter than they went in. */
static void a_fast_sender_on_its_own_clock_is_resampled(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	assert_true(resampled_packets(RATE_COMPENSATED, (int64_t)(4 * HOUR)) > 0);
}

/* Guard: audio on its video's clock gets no compensation it did not ask
 * for. */
static void audio_on_the_video_clock_is_not_resampled(void **state)
{
	UNUSED_PARAMETER(state);
	if (!obs_up)
		skip();
	assert_int_equal(resampled_packets(RATE_COMPENSATED, 0), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(audio_hours_ahead_of_video_is_heard, case_teardown),
		cmocka_unit_test_teardown(audio_hours_behind_video_is_heard, case_teardown),
		cmocka_unit_test_teardown(small_packets_audio_ahead_of_video_is_heard, case_teardown),
		cmocka_unit_test_teardown(audio_on_the_video_clock_is_heard, case_teardown),
		cmocka_unit_test_teardown(fast_sender_clock_does_not_build_up_delay, case_teardown),
		cmocka_unit_test_teardown(a_fast_sender_on_its_own_clock_is_resampled, case_teardown),
		cmocka_unit_test_teardown(audio_on_the_video_clock_is_not_resampled, case_teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
