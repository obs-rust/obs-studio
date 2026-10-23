/* The balance and Force Mono gates of process_audio() through the real
 * libobs: a stereo source sends planar float packets at the output's rate
 * (so no resampler), and an audio capture callback reads back what libobs
 * made of them, bit for bit:
 *
 *   - balance 0.5 (centred): the samples come out as they went in;
 *   - balance 0.25: the sine law, left times sinf(0.75 * pi / 2) and right
 *     times sinf(0.25 * pi / 2), with the C's float and double casts;
 *   - Force Mono: both channels are the mean, (left + right) * 0.5f;
 *   - balance 0.25 and Force Mono: balanced first, then the mean.
 *
 * test_source_audio_input_steps characterizes the steps themselves. The
 * source is kept out of the mix, and the callback runs on the thread that
 * calls obs_source_output_audio(), so the test reads what it sent. */
#ifndef _USE_MATH_DEFINES
#define _USE_MATH_DEFINES
#endif

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <string.h>

#include <obs-internal.h>
#include <util/platform.h>

#ifndef M_PI
#define M_PI 3.14159265358979323846
#endif

#define SOURCE_ID "test_source_audio_balance_mono_input"
#define RATE 48000
#define FRAMES 64
#define SEC 1000000000ULL

static int obs_up;

static const char *bm_name(void *unused)
{
	(void)unused;
	return "test source audio balance mono";
}

static void *bm_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void bm_destroy(void *data)
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

struct capture {
	uint32_t packets;
	uint32_t frames;
	float left[FRAMES];
	float right[FRAMES];
};

static void capture_cb(void *param, obs_source_t *source, const struct audio_data *audio, bool muted)
{
	struct capture *cap = param;
	(void)source;
	(void)muted;

	cap->packets++;
	cap->frames = audio->frames;
	if (audio->frames != FRAMES || !audio->data[0] || !audio->data[1])
		return;
	memcpy(cap->left, audio->data[0], sizeof(cap->left));
	memcpy(cap->right, audio->data[1], sizeof(cap->right));
}

static float left_in[FRAMES];
static float right_in[FRAMES];

static uint32_t bits_of(float value)
{
	uint32_t bits;
	memcpy(&bits, &value, sizeof(bits));
	return bits;
}

/* Sends one packet with the given balance and flags and returns what the
 * capture callback saw. */
static struct capture send_packet(float balance, uint32_t flags)
{
	struct capture cap = {0};
	obs_source_t *source = obs_source_create_private(SOURCE_ID, "balance mono", NULL);
	assert_non_null(source);
	/* Keep the mixer away from it. */
	obs_source_remove(source);
	obs_source_set_balance_value(source, balance);
	obs_source_set_flags(source, flags);
	obs_source_add_audio_capture_callback(source, capture_cb, &cap);

	struct obs_source_audio audio = {
		.data = {(uint8_t *)left_in, (uint8_t *)right_in},
		.frames = FRAMES,
		.speakers = SPEAKERS_STEREO,
		.format = AUDIO_FORMAT_FLOAT_PLANAR,
		.samples_per_sec = RATE,
		/* far from os_gettime_ns(), so libobs never mistakes it for its
		 * own clock */
		.timestamp = os_gettime_ns() + 3600 * SEC,
	};
	obs_source_output_audio(source, &audio);

	assert_null(source->resampler);
	obs_source_remove_audio_capture_callback(source, capture_cb, &cap);
	obs_source_release(source);

	assert_int_equal(cap.packets, 1);
	assert_int_equal(cap.frames, FRAMES);
	return cap;
}

static void check_samples(const struct capture *cap, const float *left, const float *right)
{
	for (size_t i = 0; i < FRAMES; i++) {
		if (bits_of(cap->left[i]) != bits_of(left[i]) || bits_of(cap->right[i]) != bits_of(right[i])) {
			fprintf(stderr, "frame %zu: got %a %a, expected %a %a\n", i, (double)cap->left[i],
				(double)cap->right[i], (double)left[i], (double)right[i]);
			fail();
		}
	}
}

static void centred_is_unchanged(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct capture cap = send_packet(0.5f, 0);
	check_samples(&cap, left_in, right_in);
}

static float left_balanced[FRAMES];
static float right_balanced[FRAMES];

static void balance_quarter_is_the_sine_law(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct capture cap = send_packet(0.25f, 0);
	check_samples(&cap, left_balanced, right_balanced);
}

static void force_mono_is_the_mean(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	float mono[FRAMES];
	for (size_t i = 0; i < FRAMES; i++) {
		mono[i] = left_in[i];
		mono[i] += right_in[i];
		mono[i] *= 1.0f / 2.0f;
	}

	struct capture cap = send_packet(0.5f, OBS_SOURCE_FLAG_FORCE_MONO);
	check_samples(&cap, mono, mono);
}

static void balance_then_mono(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	float mono[FRAMES];
	for (size_t i = 0; i < FRAMES; i++) {
		mono[i] = left_balanced[i];
		mono[i] += right_balanced[i];
		mono[i] *= 1.0f / 2.0f;
	}

	struct capture cap = send_packet(0.25f, OBS_SOURCE_FLAG_FORCE_MONO);
	check_samples(&cap, mono, mono);
}

static int setup(void **state)
{
	(void)state;
	base_set_log_handler(quiet_log, NULL);

	for (size_t i = 0; i < FRAMES; i++) {
		left_in[i] = 0.9f * (float)sin(2.0 * M_PI * 1000.0 * (double)i / RATE);
		right_in[i] = -0.6f * (float)cos(2.0 * M_PI * 440.0 * (double)i / RATE);
	}
	/* the sine law at 0.25, with the C's float and double casts */
	const float balance = 0.25f;
	const float left_gain = sinf((float)((double)(1.0f - balance) * (M_PI / 2.0)));
	const float right_gain = sinf((float)((double)balance * (M_PI / 2.0)));
	for (size_t i = 0; i < FRAMES; i++) {
		left_balanced[i] = left_in[i] * left_gain;
		right_balanced[i] = right_in[i] * right_gain;
	}

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
		.get_name = bm_name,
		.create = bm_create,
		.destroy = bm_destroy,
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
		cmocka_unit_test(centred_is_unchanged),
		cmocka_unit_test(balance_quarter_is_the_sine_law),
		cmocka_unit_test(force_mono_is_the_mean),
		cmocka_unit_test(balance_then_mono),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
