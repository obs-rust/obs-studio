/* Regression test for obsproject/obs-studio#7063 (reported by norihiro):
 * when a Media Source plays a file whose sample rate differs from OBS's
 * (44.1 kHz into 48 kHz) and is started again, the first thing heard is
 * the end of the previous playback, then the file from its beginning.
 *
 * libobs converts such audio with the source's resampler, which holds back
 * the last few samples of every packet for its filter. When the source
 * stops sending audio (the file ended) those samples stay in the resampler,
 * and the next packet, however much later it comes, is output after them.
 *
 * The test drives the real libobs input path (obs_source_output_audio and
 * the source's resampler) with a 44.1 kHz tone followed by silence, and
 * reads back what libobs made of each packet:
 *
 *   - after a gap (the file played to its end and was started again later),
 *     the silence must come out silent;
 *   - control: when the silence follows the tone without a gap, the end of
 *     the tone is part of the same stream and must still come out.
 *
 * The source is kept out of the mix, so the audio thread never touches its
 * buffers behind the test's back. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>

#include <obs-internal.h>
#include <util/platform.h>
#include <util/util_uint64.h>

#define SOURCE_ID "test_audio_restart_resampler_tail_input"

#define OBS_RATE 48000
#define FILE_RATE 44100
#define FRAMES 441 /* 10 ms at the file's rate */
#define SEC 1000000000ULL
#define MSEC 1000000ULL
#define TONE_HZ 1000.0
#define AMPLITUDE 0.9f
/* Silence through the resampler stays well below this. */
#define SILENT 0.001f

static int obs_up;
/* Samples above SILENT in what libobs made of the last packet. */
static uint32_t last_loud;

static const char *tail_name(void *unused)
{
	(void)unused;
	return "test audio restart resampler tail";
}

static void *tail_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void tail_destroy(void *data)
{
	(void)data;
}

static void quiet_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (level > LOG_WARNING)
		return;
	vfprintf(stderr, format, args);
	fputc('\n', stderr);
}

struct player {
	obs_source_t *source;
	uint64_t base_ts;
	uint64_t frames_out;
	float buf[FRAMES];
};

/* Sends one packet of the file at time @ts and returns the loudest sample
 * libobs made of it. */
static float send_packet(struct player *p, bool tone, uint64_t ts)
{
	for (size_t i = 0; i < FRAMES; i++) {
		double t = (double)(p->frames_out + i) / FILE_RATE;
		p->buf[i] = tone ? AMPLITUDE * (float)sin(2.0 * 3.14159265358979323846 * TONE_HZ * t) : 0.0f;
	}
	p->frames_out += FRAMES;

	struct obs_source_audio audio = {
		.data = {(uint8_t *)p->buf, (uint8_t *)p->buf},
		.frames = FRAMES,
		.speakers = SPEAKERS_STEREO,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.samples_per_sec = FILE_RATE,
		.timestamp = ts,
	};
	obs_source_output_audio(p->source, &audio);

	float peak = 0.0f;
	const float *out = (const float *)p->source->audio_data.data[0];
	last_loud = 0;
	for (uint32_t i = 0; i < p->source->audio_data.frames; i++) {
		if (fabsf(out[i]) > peak)
			peak = fabsf(out[i]);
		if (fabsf(out[i]) >= SILENT)
			last_loud++;
	}
	return peak;
}

static uint64_t packet_ts(struct player *p, uint64_t gap)
{
	return p->base_ts + gap + util_mul_div64(p->frames_out, SEC, FILE_RATE);
}

static struct player start_player(void)
{
	struct player p = {0};

	p.source = obs_source_create_private(SOURCE_ID, "media file", NULL);
	assert_non_null(p.source);
	/* Keep the mixer away from it. */
	obs_source_remove(p.source);
	/* Stamps far from os_gettime_ns(), so libobs never mistakes them for
	 * its own clock. */
	p.base_ts = os_gettime_ns() + 3600 * SEC;
	return p;
}

/* Plays half a second of tone and returns the peak libobs made of the first
 * packet of silence, sent @gap after the tone ended. */
static float silence_after_tone(uint64_t gap)
{
	struct player p = start_player();
	float peak;

	for (int i = 0; i < 50; i++)
		send_packet(&p, true, packet_ts(&p, 0));

	assert_non_null(p.source->resampler);
	peak = send_packet(&p, false, packet_ts(&p, gap));

	obs_source_release(p.source);
	return peak;
}

/* The file ended, and was started again a second later: the restart must
 * not begin with the end of the previous playback. */
static void restart_starts_silent(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	float peak = silence_after_tone(1 * SEC);
	fprintf(stderr, "restart after 1 s: first packet peak %.4f, %u samples of the old tone\n", peak, last_loud);
	assert_true(peak < SILENT);
}

/* Started again soon after it ended (100 ms, beyond libobs's 70 ms timestamp
 * smoothing): still a new playback. */
static void quick_restart_starts_silent(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	float peak = silence_after_tone(100 * MSEC);
	fprintf(stderr, "restart after 100 ms: first packet peak %.4f, %u samples of the old tone\n", peak, last_loud);
	assert_true(peak < SILENT);
}

/* Control: silence that continues the tone is the same stream, and the end
 * of the tone the resampler held back belongs in front of it. */
static void continuous_stream_keeps_its_end(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	float peak = silence_after_tone(0);
	fprintf(stderr, "continuous: first silent packet peak %.4f, %u samples of the tone\n", peak, last_loud);
	assert_true(peak > 0.1f);
}

/* Control: small timestamp jitter (within the 70 ms smoothing) is the same
 * stream too. */
static void jittered_stream_keeps_its_end(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	float peak = silence_after_tone(20 * MSEC);
	fprintf(stderr, "20 ms late: first silent packet peak %.4f, %u samples of the tone\n", peak, last_loud);
	assert_true(peak > 0.1f);
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		/* obs_startup() needs a display on Linux; skip without one. */
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_audio_info oai = {.samples_per_sec = OBS_RATE, .speakers = SPEAKERS_STEREO};
	if (!obs_reset_audio(&oai)) {
		fprintf(stderr, "obs_reset_audio failed\n");
		return -1;
	}

	struct obs_source_info info = {
		.id = SOURCE_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_AUDIO,
		.get_name = tail_name,
		.create = tail_create,
		.destroy = tail_destroy,
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
		cmocka_unit_test(restart_starts_silent),
		cmocka_unit_test(quick_restart_starts_silent),
		cmocka_unit_test(continuous_stream_keeps_its_end),
		cmocka_unit_test(jittered_stream_keeps_its_end),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
