#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "media-io/audio-resampler-drift.h"

/* Characterization of the libswresample resampler's output plan, moved from
 * libobs/media-io/audio-resampler-ffmpeg.c to
 * libobs/media-io/audio-resampler-plan.c: the channel mask of each speaker
 * layout, the output channels, whether the output is planar, the input
 * channels, the 4.1 layouts and the mono upmix gate. No swr context: the test
 * calls only the moved functions, which are hidden in libobs, so this
 * characterizes that C source. */

/* Speaker layout and audio format values outside their enums, as an int
 * cast to the enum would give them. 7 is the gap before SPEAKERS_7POINT1. */
static const int odd_layouts[] = {-1, 7, 9, 100, INT_MAX, INT_MIN};
static const int odd_formats[] = {-1, 9, 100, INT_MAX, INT_MIN};

#define N(a) (sizeof(a) / sizeof((a)[0]))

/* What audio_resampler_create() sets up from the moved functions, asked as it
 * asks them: the planes from the output channels when the output is planar;
 * the input channels and the 4.1 layouts on the newer libswresample. */
struct plan {
	uint32_t output_ch;
	uint32_t output_planes;
	int input_channels;
	bool input_4point1;
	bool output_4point1;
};

static void check_plan(struct plan got, uint32_t output_ch, uint32_t output_planes, int input_channels,
		       bool input_4point1, bool output_4point1, int src, int dst, int format)
{
	if (got.output_ch != output_ch || got.output_planes != output_planes || got.input_channels != input_channels ||
	    got.input_4point1 != input_4point1 || got.output_4point1 != output_4point1) {
		fprintf(stderr, "plan(%d, %d, %d): {%u, %u, %d, %d, %d}, expected {%u, %u, %d, %d, %d}\n", src, dst,
			format, got.output_ch, got.output_planes, got.input_channels, (int)got.input_4point1,
			(int)got.output_4point1, output_ch, output_planes, input_channels, (int)input_4point1,
			(int)output_4point1);
		fail();
	}
}

static struct plan plan_of(int src, int dst, int format)
{
	struct plan p;

	p.output_ch = audio_resampler_output_channels((enum speaker_layout)dst);
	p.output_planes = audio_resampler_planar_output((enum audio_format)format) ? p.output_ch : 1;
	p.input_channels = audio_resampler_input_channels((enum speaker_layout)src);
	p.input_4point1 = audio_resampler_is_4point1((enum speaker_layout)src);
	p.output_4point1 = audio_resampler_is_4point1((enum speaker_layout)dst);
	return p;
}

static void constants(void **state)
{
	(void)state;
	assert_int_equal(AUDIO_RESAMPLER_CH_LAYOUT_MONO, 0x4);
	assert_int_equal(AUDIO_RESAMPLER_CH_LAYOUT_STEREO, 0x3);
	assert_int_equal(AUDIO_RESAMPLER_CH_LAYOUT_SURROUND, 0x7);
	assert_int_equal(AUDIO_RESAMPLER_CH_LAYOUT_4POINT0, 0x107);
	assert_int_equal(AUDIO_RESAMPLER_CH_LAYOUT_4POINT1, 0x10f);
	assert_int_equal(AUDIO_RESAMPLER_CH_LAYOUT_5POINT1_BACK, 0x3f);
	assert_int_equal(AUDIO_RESAMPLER_CH_LAYOUT_7POINT1, 0x63f);
}

static void layout_masks(void **state)
{
	(void)state;
	assert_int_equal(audio_resampler_layout_mask(SPEAKERS_UNKNOWN), 0);
	assert_int_equal(audio_resampler_layout_mask(SPEAKERS_MONO), AUDIO_RESAMPLER_CH_LAYOUT_MONO);
	assert_int_equal(audio_resampler_layout_mask(SPEAKERS_STEREO), AUDIO_RESAMPLER_CH_LAYOUT_STEREO);
	/* SPEAKERS_2POINT1 is FL, FR and LFE, but its mask is surround: FL, FR
	 * and FC (characterized, not endorsed) */
	assert_int_equal(audio_resampler_layout_mask(SPEAKERS_2POINT1), AUDIO_RESAMPLER_CH_LAYOUT_SURROUND);
	assert_int_equal(audio_resampler_layout_mask(SPEAKERS_4POINT0), AUDIO_RESAMPLER_CH_LAYOUT_4POINT0);
	assert_int_equal(audio_resampler_layout_mask(SPEAKERS_4POINT1), AUDIO_RESAMPLER_CH_LAYOUT_4POINT1);
	assert_int_equal(audio_resampler_layout_mask(SPEAKERS_5POINT1), AUDIO_RESAMPLER_CH_LAYOUT_5POINT1_BACK);
	assert_int_equal(audio_resampler_layout_mask(SPEAKERS_7POINT1), AUDIO_RESAMPLER_CH_LAYOUT_7POINT1);

	for (size_t i = 0; i < N(odd_layouts); i++)
		assert_int_equal(audio_resampler_layout_mask((enum speaker_layout)odd_layouts[i]), 0);
}

static void output_channels(void **state)
{
	(void)state;
	static const struct {
		int layout;
		uint32_t channels;
	} cases[] = {
		{SPEAKERS_UNKNOWN, 0}, {SPEAKERS_MONO, 1},    {SPEAKERS_STEREO, 2},  {SPEAKERS_2POINT1, 3},
		{SPEAKERS_4POINT0, 4}, {SPEAKERS_4POINT1, 5}, {SPEAKERS_5POINT1, 6}, {SPEAKERS_7POINT1, 8},
	};

	for (size_t i = 0; i < N(cases); i++) {
		struct plan p = plan_of(SPEAKERS_STEREO, cases[i].layout, AUDIO_FORMAT_FLOAT_PLANAR);
		assert_int_equal(p.output_ch, cases[i].channels);
		assert_int_equal(p.output_planes, cases[i].channels);
	}
	for (size_t i = 0; i < N(odd_layouts); i++) {
		struct plan p = plan_of(SPEAKERS_STEREO, odd_layouts[i], AUDIO_FORMAT_FLOAT_PLANAR);
		assert_int_equal(p.output_ch, 0);
		assert_int_equal(p.output_planes, 0);
	}
}

static void output_planes(void **state)
{
	(void)state;
	static const int packed[] = {AUDIO_FORMAT_UNKNOWN, AUDIO_FORMAT_U8BIT, AUDIO_FORMAT_16BIT, AUDIO_FORMAT_32BIT,
				     AUDIO_FORMAT_FLOAT};
	static const int planar[] = {AUDIO_FORMAT_U8BIT_PLANAR, AUDIO_FORMAT_16BIT_PLANAR, AUDIO_FORMAT_32BIT_PLANAR,
				     AUDIO_FORMAT_FLOAT_PLANAR};

	for (int dst = SPEAKERS_UNKNOWN; dst <= SPEAKERS_7POINT1; dst++) {
		uint32_t ch = get_audio_channels((enum speaker_layout)dst);

		for (size_t i = 0; i < N(packed); i++)
			check_plan(plan_of(SPEAKERS_STEREO, dst, packed[i]), ch, 1, 2, false, dst == SPEAKERS_4POINT1,
				   SPEAKERS_STEREO, dst, packed[i]);
		for (size_t i = 0; i < N(planar); i++)
			check_plan(plan_of(SPEAKERS_STEREO, dst, planar[i]), ch, ch, 2, false, dst == SPEAKERS_4POINT1,
				   SPEAKERS_STEREO, dst, planar[i]);
		/* a format outside the enum is packed */
		for (size_t i = 0; i < N(odd_formats); i++)
			check_plan(plan_of(SPEAKERS_STEREO, dst, odd_formats[i]), ch, 1, 2, false,
				   dst == SPEAKERS_4POINT1, SPEAKERS_STEREO, dst, odd_formats[i]);
	}

	/* packed with no output channels still has one plane (characterized,
	 * not endorsed) */
	check_plan(plan_of(SPEAKERS_STEREO, SPEAKERS_UNKNOWN, AUDIO_FORMAT_FLOAT), 0, 1, 2, false, false,
		   SPEAKERS_STEREO, SPEAKERS_UNKNOWN, AUDIO_FORMAT_FLOAT);
}

static void input_channels(void **state)
{
	(void)state;
	static const struct {
		int layout;
		int channels;
	} cases[] = {
		/* the newer libswresample asks the default layout of 0 channels
		 * for an unknown input (characterized, not endorsed) */
		{SPEAKERS_UNKNOWN, 0}, {SPEAKERS_MONO, 1},    {SPEAKERS_STEREO, 2},  {SPEAKERS_2POINT1, 3},
		{SPEAKERS_4POINT0, 4}, {SPEAKERS_4POINT1, 5}, {SPEAKERS_5POINT1, 6}, {SPEAKERS_7POINT1, 8},
	};

	for (size_t i = 0; i < N(cases); i++)
		assert_int_equal(plan_of(cases[i].layout, SPEAKERS_STEREO, AUDIO_FORMAT_FLOAT).input_channels,
				 cases[i].channels);
	for (size_t i = 0; i < N(odd_layouts); i++)
		assert_int_equal(plan_of(odd_layouts[i], SPEAKERS_STEREO, AUDIO_FORMAT_FLOAT).input_channels, 0);
}

static void four_point_one(void **state)
{
	(void)state;
	struct plan p;

	p = plan_of(SPEAKERS_4POINT1, SPEAKERS_STEREO, AUDIO_FORMAT_FLOAT);
	assert_true(p.input_4point1);
	assert_false(p.output_4point1);

	p = plan_of(SPEAKERS_STEREO, SPEAKERS_4POINT1, AUDIO_FORMAT_FLOAT);
	assert_false(p.input_4point1);
	assert_true(p.output_4point1);

	p = plan_of(SPEAKERS_4POINT1, SPEAKERS_4POINT1, AUDIO_FORMAT_FLOAT_PLANAR);
	check_plan(p, 5, 5, 5, true, true, SPEAKERS_4POINT1, SPEAKERS_4POINT1, AUDIO_FORMAT_FLOAT_PLANAR);

	/* only 4.1: not 4.0, not five channels by another layout */
	for (int l = -1; l <= 9; l++) {
		if (l == SPEAKERS_4POINT1)
			continue;
		p = plan_of(l, l, AUDIO_FORMAT_FLOAT);
		assert_false(p.input_4point1);
		assert_false(p.output_4point1);
	}
}

static void mono_into_each_layout(void **state)
{
	(void)state;

	/* the input is mono on both libswresample paths: its mask is mono, and
	 * the default layout of its one channel is mono */
	assert_int_equal(audio_resampler_layout_mask(SPEAKERS_MONO), AUDIO_RESAMPLER_CH_LAYOUT_MONO);

	for (int dst = -1; dst <= 9; dst++) {
		struct plan p = plan_of(SPEAKERS_MONO, dst, AUDIO_FORMAT_FLOAT_PLANAR);
		uint32_t ch = get_audio_channels((enum speaker_layout)dst);

		check_plan(p, ch, ch, 1, false, dst == SPEAKERS_4POINT1, SPEAKERS_MONO, dst, AUDIO_FORMAT_FLOAT_PLANAR);
		/* upmixed to two channels or more; not to mono, unknown or a layout
		 * outside the enum */
		assert_int_equal(audio_resampler_upmixes_mono(p.output_ch), ch > 1);
	}
}

static void upmix_gate(void **state)
{
	(void)state;
	static const uint32_t channels[] = {0, 1, 2, 3, 5, 8, 9, 64, UINT32_MAX};

	/* asked only for a mono input */
	for (size_t i = 0; i < N(channels); i++)
		assert_int_equal(audio_resampler_upmixes_mono(channels[i]), channels[i] > 1);
}

/* Every pair of layouts from -2 to 10 and every format from -2 to 10 against
 * the expressions audio_resampler_create() wrote. */
static void every_input(void **state)
{
	(void)state;

	for (int src = -2; src <= 10; src++)
		for (int dst = -2; dst <= 10; dst++)
			for (int format = -2; format <= 10; format++) {
				uint32_t output_ch = get_audio_channels((enum speaker_layout)dst);
				uint32_t output_planes = is_audio_planar((enum audio_format)format) ? output_ch : 1;
				int input_ch = (int)get_audio_channels((enum speaker_layout)src);

				check_plan(plan_of(src, dst, format), output_ch, output_planes, input_ch,
					   src == SPEAKERS_4POINT1, dst == SPEAKERS_4POINT1, src, dst, format);
			}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(layout_masks),
		cmocka_unit_test(output_channels),
		cmocka_unit_test(output_planes),
		cmocka_unit_test(input_channels),
		cmocka_unit_test(four_point_one),
		cmocka_unit_test(mono_into_each_layout),
		cmocka_unit_test(upmix_gate),
		cmocka_unit_test(every_input),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
