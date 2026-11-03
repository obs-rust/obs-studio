/* Regression test for obsproject/obs-studio#4600 (reported by floele, with
 * measurements from Basvg, norihiro and many others; JACK reports by ogmkp
 * on #12835, also #11083): audio from a capture device drops out for a
 * moment every few minutes.
 *
 * A sound card runs on its own crystal, which is never exactly at its
 * nominal rate: Basvg measured one 140 ppm fast. The capture plugins stamp
 * each packet with the system clock when it arrives, and libobs then places
 * the packet by counting samples from the previous one as long as the stamp
 * stays within 70 ms (TS_SMOOTHING_THRESHOLD in obs-source.c) of where the
 * count says it should be. A device that delivers a few samples per second
 * too many or too few moves its stamps away from the count by that much
 * every second, so after 70 ms / 140 ppm = 500 s libobs gives up counting
 * and places the packet at its stamp: 70 ms of audio are overwritten (fast
 * device) or 70 ms of silence are inserted (slow device), and the cycle
 * starts again.
 *
 * The test drives the real libobs input path (obs_source_output_audio, the
 * source's resampler and the timestamp smoothing) with a sine wave from a
 * simulated device whose clock is off by a few hundred ppm, stamped like
 * linux-jack does, on a virtual clock so hours of audio take seconds. It
 * reads back what libobs buffered for the mixer and counts packets that were
 * not appended seamlessly, and clicks in the buffered waveform. The audio
 * thread waits in a task while it runs, so it never consumes or resets the
 * source's buffer behind the test's back.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>

#include <obs-internal.h>
#include <util/platform.h>
#include <util/util_uint64.h>

#define SOURCE_ID "test_audio_clock_drift_input"

#define RATE 48000
#define FRAMES 480 /* a 10 ms period */
#define SEC 1000000000ULL
#define MSEC 1000000ULL
#define TONE_HZ 997.0
#define AMPLITUDE 0.5
/* Largest second difference of the tone is A * (2 pi f / fs)^2 ~= 0.0085. */
#define CLICK_THRESHOLD 0.03
/* Keep about as much buffered as the mixer would. */
#define KEEP_FRAMES 2400
#define DRAIN_ABOVE 4800
/* libobs resyncs once the audio is 70 ms off its timestamps; stay well
 * clear of that. */
#define MAX_LAG_MS 35.0

static int obs_up;

static const char *drift_name(void *unused)
{
	(void)unused;
	return "test audio clock drift";
}

static void *drift_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void drift_destroy(void *data)
{
	(void)data;
}

struct device {
	double ppm;
	/* Arrival jitter in ns, uniform in [-jitter, +jitter]. */
	uint64_t jitter;
	uint32_t lcg;
	uint64_t base_ts;
	uint64_t frames_out;
	float buf[FRAMES];
};

struct result {
	size_t packets;
	size_t dropouts;
	double first_dropout_s;
	size_t clicks;
	/* Buffered samples that differ from what the device sent, for a
	 * device that needs no correction. */
	size_t altered;
	uint64_t buffered_frames;
	bool resampled;
	double max_d2;
	/* Largest distance between where the buffered audio has got to and
	 * where the device's timestamps say it should be. */
	double max_lag_ms;
	/* d2 detector state */
	double prev[2];
	int have_prev;
};

/* A whole number of cycles per second, so one second of it repeats. */
static float tone_table[RATE];

static void fill_tone_table(void)
{
	for (size_t n = 0; n < RATE; n++)
		tone_table[n] = (float)(AMPLITUDE * sin(2.0 * 3.14159265358979323846 * TONE_HZ * (double)n / RATE));
}

static float tone(uint64_t n)
{
	return tone_table[n % RATE];
}

static void quiet_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (level > LOG_WARNING)
		return;
	vfprintf(stderr, format, args);
	fputc('\n', stderr);
}

static int64_t jitter(struct device *dev)
{
	if (!dev->jitter)
		return 0;
	dev->lcg = dev->lcg * 1664525u + 1013904223u;
	return (int64_t)((uint64_t)(dev->lcg >> 8) % (2 * dev->jitter + 1)) - (int64_t)dev->jitter;
}

static void check_buffered(struct result *r, const float *data, size_t frames, bool exact, uint64_t first_index)
{
	for (size_t i = 0; i < frames; i++) {
		double x = data[i];

		if (exact && data[i] != tone(first_index + i))
			r->altered++;

		if (r->have_prev >= 2) {
			double d2 = fabs(x - 2.0 * r->prev[1] + r->prev[0]);
			if (d2 > r->max_d2)
				r->max_d2 = d2;
			if (d2 > CLICK_THRESHOLD)
				r->clicks++;
		}
		r->prev[0] = r->prev[1];
		r->prev[1] = x;
		if (r->have_prev < 2)
			r->have_prev++;
	}
}

static void drain(obs_source_t *source, struct result *r, size_t keep, bool exact)
{
	size_t size = source->audio_input_buf[0].size;
	size_t keep_bytes = keep * sizeof(float);
	float tmp[DRAIN_ABOVE * 4];

	if (size <= keep_bytes)
		return;

	size_t pop = size - keep_bytes;
	size_t frames = pop / sizeof(float);

	if (frames > sizeof(tmp) / sizeof(tmp[0])) {
		fprintf(stderr, "buffer grew to %zu frames\n", frames);
		fail();
	}

	for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++) {
		if (!source->audio_input_buf[ch].size)
			continue;
		if (ch == 0) {
			deque_pop_front(&source->audio_input_buf[0], tmp, pop);
			check_buffered(r, tmp, frames, exact, r->buffered_frames);
		} else {
			deque_pop_front(&source->audio_input_buf[ch], NULL, pop);
		}
	}
	r->buffered_frames += frames;
	source->audio_ts += util_mul_div64(frames, SEC, RATE);
}

struct audio_thread_park {
	os_event_t *parked;
	os_event_t *resume;
};

static void park_audio_thread_task(void *param)
{
	struct audio_thread_park *park = param;

	os_event_signal(park->parked);
	os_event_wait(park->resume);
}

/* The audio thread discards from every source on its list, removed from the
 * mix or not. The first stamp maps to the real clock, so a stall in the first
 * packets leaves the buffer behind that clock, and a tick consumes it (#3576).
 * Returns with the audio thread in park_audio_thread_task(). */
static void park_audio_thread(struct audio_thread_park *park)
{
	assert_int_equal(os_event_init(&park->parked, OS_EVENT_TYPE_AUTO), 0);
	assert_int_equal(os_event_init(&park->resume, OS_EVENT_TYPE_AUTO), 0);
	obs_queue_task(OBS_TASK_AUDIO, park_audio_thread_task, park, false);
	os_event_wait(park->parked);
}

static void audio_thread_task_done(void *param)
{
	UNUSED_PARAMETER(param);
}

static void resume_audio_thread(struct audio_thread_park *park)
{
	os_event_signal(park->resume);
	/* Tasks run in order: this one returns after the park task has ended. */
	obs_queue_task(OBS_TASK_AUDIO, audio_thread_task_done, NULL, true);
	os_event_destroy(park->parked);
	os_event_destroy(park->resume);
}

/* Feed @seconds of audio from a device @ppm off its nominal rate. */
static struct result run_device(double ppm, uint64_t jitter_ns, unsigned seconds, bool compensate)
{
	struct result r = {0};
	struct device dev = {.ppm = ppm, .jitter = jitter_ns, .lcg = 12345};
	obs_source_t *source = obs_source_create_private(SOURCE_ID, "drifting device", NULL);

	if (!source) {
		fprintf(stderr, "could not create the test source\n");
		fail();
	}
	obs_source_set_async_compensation(source, compensate);
	/* Keep the mixer away from it: the test consumes the buffer itself. */
	obs_source_remove(source);

	struct audio_thread_park park;
	park_audio_thread(&park);

	/* Stamps far from os_gettime_ns(), so libobs never mistakes them for
	 * its own clock while the virtual clock runs ahead of the real one. */
	dev.base_ts = os_gettime_ns() + 3600 * SEC;
	double device_rate = RATE * (1.0 + ppm * 1e-6);
	uint64_t packets = (uint64_t)seconds * RATE / FRAMES;
	uint64_t out_frames = 0;
	uint64_t first_ts = 0;

	for (uint64_t p = 0; p < packets; p++) {
		for (size_t i = 0; i < FRAMES; i++)
			dev.buf[i] = tone(dev.frames_out + i);
		dev.frames_out += FRAMES;

		/* The period ended when the device had produced frames_out
		 * samples; linux-jack stamps it one nominal period earlier. */
		uint64_t end_ns = (uint64_t)((double)dev.frames_out / device_rate * 1e9);
		uint64_t ts = dev.base_ts + end_ns - util_mul_div64(FRAMES, SEC, RATE) + jitter(&dev);

		struct obs_source_audio audio = {
			.data = {(uint8_t *)dev.buf, (uint8_t *)dev.buf},
			.frames = FRAMES,
			.speakers = SPEAKERS_STEREO,
			.format = AUDIO_FORMAT_FLOAT_PLANAR,
			.samples_per_sec = RATE,
			.timestamp = ts,
		};

		pthread_mutex_lock(&source->audio_buf_mutex);
		size_t before = source->audio_input_buf[0].size;
		pthread_mutex_unlock(&source->audio_buf_mutex);

		obs_source_output_audio(source, &audio);

		pthread_mutex_lock(&source->audio_buf_mutex);
		size_t after = source->audio_input_buf[0].size;
		size_t added = source->audio_data.frames * sizeof(float);

		out_frames += source->audio_data.frames;
		if (p > 0) {
			double lag = ((double)out_frames * 1e9 / RATE - (double)(ts - first_ts) -
				      (double)FRAMES * 1e9 / RATE) /
				     1e6;
			if (fabs(lag) > r.max_lag_ms)
				r.max_lag_ms = fabs(lag);
		} else {
			first_ts = ts;
		}

		/* The first packet starts the buffer; every other one must
		 * continue it exactly. */
		if (p > 0 && after != before + added) {
			if (!r.dropouts)
				r.first_dropout_s = (double)end_ns / 1e9;
			r.dropouts++;
		}
		if (after > DRAIN_ABOVE * sizeof(float))
			drain(source, &r, KEEP_FRAMES, ppm == 0.0);
		pthread_mutex_unlock(&source->audio_buf_mutex);
		r.packets++;
	}

	pthread_mutex_lock(&source->audio_buf_mutex);
	drain(source, &r, 0, ppm == 0.0);
	r.resampled = source->resampler != NULL;
	pthread_mutex_unlock(&source->audio_buf_mutex);

	resume_audio_thread(&park);

	obs_source_release(source);
	return r;
}

static void report(const char *name, double ppm, unsigned seconds, const struct result *r)
{
	fprintf(stderr,
		"%s: %+.0f ppm, %u s simulated (%zu packets): %zu dropouts (first at %.1f s), %zu clicks (max d2 %.5f), "
		"%zu altered samples, %llu frames buffered, max lag %.1f ms\n",
		name, ppm, seconds, r->packets, r->dropouts, r->first_dropout_s, r->clicks, r->max_d2, r->altered,
		(unsigned long long)r->buffered_frames, r->max_lag_ms);
}

static void expect_seamless(const char *name, double ppm, uint64_t jitter_ns, unsigned seconds)
{
	struct result r = run_device(ppm, jitter_ns, seconds, true);

	report(name, ppm, seconds, &r);
	if (r.dropouts || r.clicks) {
		fprintf(stderr, "%s: audio from a %+.0f ppm device dropped out\n", name, ppm);
		fail();
	}
	if (r.max_lag_ms > MAX_LAG_MS) {
		fprintf(stderr, "%s: audio strayed %.1f ms from its timestamps\n", name, r.max_lag_ms);
		fail();
	}
}

/* Basvg's device: a dropout every ~8 minutes. Two hours of it. */
static void fast_device_clock_has_no_dropouts(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	expect_seamless("fast device", 140.0, 0, 2 * 3600);
}

static void slow_device_clock_has_no_dropouts(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	expect_seamless("slow device", -140.0, 0, 2 * 3600);
}

/* Arrival times wobble with scheduling; the correction must not follow the
 * wobble, only the drift. */
static void jittery_drifting_device_has_no_dropouts(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	expect_seamless("jittery fast device", 140.0, 4 * MSEC, 2 * 3600);
	expect_seamless("jittery slow device", -300.0, 4 * MSEC, 3600);
}

static void far_off_device_has_no_dropouts(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	expect_seamless("far off fast device", 800.0, 1 * MSEC, 3600);
	expect_seamless("far off slow device", -800.0, 1 * MSEC, 3600);
}

/* A device on the system clock needs no correction: its audio must reach
 * the mixer bit for bit, jitter or not. */
static void exact_rate_device_is_untouched(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	struct result r = run_device(0.0, 4 * MSEC, 1800, true);

	report("exact device", 0.0, 1800, &r);
	if (r.dropouts || r.clicks || r.altered) {
		fprintf(stderr, "exact device: audio was altered\n");
		fail();
	}
	if (r.buffered_frames != (uint64_t)1800 * RATE) {
		fprintf(stderr, "exact device: %llu frames buffered, expected %llu\n",
			(unsigned long long)r.buffered_frames, (unsigned long long)1800 * RATE);
		fail();
	}
}

/* Sources that do not ask for it keep the old behaviour. */
static void compensation_is_opt_in(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	struct result r = run_device(140.0, 0, 1200, false);

	report("uncompensated fast device", 140.0, 1200, &r);
	if (r.resampled) {
		fprintf(stderr, "uncompensated source: audio was resampled\n");
		fail();
	}
}

static int setup(void **state)
{
	(void)state;
	fill_tone_table();
	base_set_log_handler(quiet_log, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		/* obs_startup() needs a display on Linux; skip without one. */
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		return -1;
	}

	struct obs_source_info info = {
		.id = SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = drift_name,
		.create = drift_create,
		.destroy = drift_destroy,
	};
	obs_register_source(&info);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(fast_device_clock_has_no_dropouts),
		cmocka_unit_test(slow_device_clock_has_no_dropouts),
		cmocka_unit_test(jittery_drifting_device_has_no_dropouts),
		cmocka_unit_test(far_off_device_has_no_dropouts),
		cmocka_unit_test(exact_rate_device_is_untouched),
		cmocka_unit_test(compensation_is_opt_in),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
