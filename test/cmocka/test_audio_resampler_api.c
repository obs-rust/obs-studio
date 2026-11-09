#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include "media-io/audio-resampler.h"

/* Pin the public C wrapper, not just the already-ported plan/drift helpers.
 * FFmpeg still performs conversion. No OBS startup, devices or clocks. */

static void null_resampler(void **state)
{
	(void)state;
	uint8_t sentinel = 0;
	uint8_t *output[MAX_AV_PLANES] = {&sentinel};
	uint32_t frames = 123;
	uint64_t offset = 456;

	audio_resampler_destroy(NULL);
	audio_resampler_set_compensation_error(NULL, 1000000);
	assert_false(audio_resampler_resample(NULL, output, &frames, &offset, NULL, 0));
	assert_ptr_equal(output[0], &sentinel);
	assert_int_equal(frames, 123);
	assert_int_equal(offset, 456);
}

static void invalid_sample_rate(void **state)
{
	(void)state;
	const struct resample_info valid = {48000, AUDIO_FORMAT_FLOAT, SPEAKERS_STEREO};
	const struct resample_info invalid = {0, AUDIO_FORMAT_FLOAT, SPEAKERS_STEREO};

	assert_null(audio_resampler_create(&invalid, &valid));
	assert_null(audio_resampler_create(&valid, &invalid));
}

static void unknown_format_is_s16(void **state)
{
	(void)state;
	/* Characterized, not endorsed: UNKNOWN falls back to packed signed 16-bit. */
	const struct resample_info unknown = {48000, AUDIO_FORMAT_UNKNOWN, SPEAKERS_MONO};
	const struct resample_info known = {48000, AUDIO_FORMAT_FLOAT, SPEAKERS_MONO};
	const int16_t shorts[] = {-32768, -16384, 0, 8192, 32767};
	const float floats[] = {-1.0f, -0.5f, 0.0f, 0.25f, 32767.0f / 32768.0f};
	for (size_t direction = 0; direction < 2; direction++) {
		audio_resampler_t *rs = direction == 0 ? audio_resampler_create(&known, &unknown)
						       : audio_resampler_create(&unknown, &known);
		assert_non_null(rs);
		const uint8_t *input[MAX_AV_PLANES] = {direction == 0 ? (const uint8_t *)shorts
								      : (const uint8_t *)floats};
		uint8_t *output[MAX_AV_PLANES] = {0};
		uint32_t frames = 0;
		uint64_t offset = UINT64_MAX;
		assert_true(audio_resampler_resample(rs, output, &frames, &offset, input, 5));
		assert_int_equal(frames, 5);
		assert_int_equal(offset, 0);
		assert_non_null(output[0]);
		for (uint32_t i = 0; i < frames; i++) {
			if (direction == 0)
				assert_true(((const float *)output[0])[i] == floats[i]);
			else
				assert_int_equal(((const int16_t *)output[0])[i], shorts[i]);
		}
		audio_resampler_destroy(rs);
	}
}

static void planar_to_packed_and_buffer_growth(void **state)
{
	(void)state;
	const struct resample_info src = {48000, AUDIO_FORMAT_FLOAT_PLANAR, SPEAKERS_STEREO};
	const struct resample_info dst = {48000, AUDIO_FORMAT_FLOAT, SPEAKERS_STEREO};
	audio_resampler_t *rs = audio_resampler_create(&dst, &src);
	assert_non_null(rs);
	float left[1024], right[1024];
	for (size_t i = 0; i < 1024; i++) {
		left[i] = (float)i / 1024.0f;
		right[i] = -left[i];
	}
	const uint8_t *input[MAX_AV_PLANES] = {(const uint8_t *)left, (const uint8_t *)right};
	const uint32_t sizes[] = {16, 1024, 32, 1024};
	for (size_t call = 0; call < sizeof(sizes) / sizeof(sizes[0]); call++) {
		uint8_t sentinel = 0;
		uint8_t *output[MAX_AV_PLANES] = {NULL, &sentinel};
		uint32_t frames = 0;
		uint64_t offset = UINT64_MAX;
		/* Enabling compensation at zero error must preserve the samples. */
		audio_resampler_set_compensation_error(rs, 0);
		assert_true(audio_resampler_resample(rs, output, &frames, &offset, input, sizes[call]));
		assert_int_equal(frames, sizes[call]);
		assert_int_equal(offset, 0);
		assert_non_null(output[0]);
		assert_ptr_equal(output[1], &sentinel);
		const float *packed = (const float *)output[0];
		for (uint32_t i = 0; i < frames; i++) {
			assert_true(packed[2 * i] == left[i]);
			assert_true(packed[2 * i + 1] == right[i]);
		}
	}
	audio_resampler_destroy(rs);
}

static void packed_to_planar(void **state)
{
	(void)state;
	const struct resample_info src = {48000, AUDIO_FORMAT_FLOAT, SPEAKERS_STEREO};
	const struct resample_info dst = {48000, AUDIO_FORMAT_FLOAT_PLANAR, SPEAKERS_STEREO};
	audio_resampler_t *rs = audio_resampler_create(&dst, &src);
	assert_non_null(rs);
	const float packed[] = {0.25f, -0.5f, 0.75f, -1.0f, 0.0f, 1.0f};
	const uint8_t *input[MAX_AV_PLANES] = {(const uint8_t *)packed};
	uint8_t sentinel = 0;
	uint8_t *output[MAX_AV_PLANES] = {NULL, NULL, &sentinel};
	uint32_t frames = 0;
	uint64_t offset = UINT64_MAX;
	assert_true(audio_resampler_resample(rs, output, &frames, &offset, input, 3));
	assert_int_equal(frames, 3);
	assert_int_equal(offset, 0);
	assert_non_null(output[0]);
	assert_non_null(output[1]);
	assert_ptr_equal(output[2], &sentinel);
	for (uint32_t i = 0; i < frames; i++) {
		assert_true(((const float *)output[0])[i] == packed[2 * i]);
		assert_true(((const float *)output[1])[i] == packed[2 * i + 1]);
	}
	audio_resampler_destroy(rs);
}

static void mono_upmix(void **state)
{
	(void)state;
	const struct resample_info src = {48000, AUDIO_FORMAT_FLOAT, SPEAKERS_MONO};
	const struct resample_info dst = {48000, AUDIO_FORMAT_FLOAT_PLANAR, SPEAKERS_STEREO};
	audio_resampler_t *rs = audio_resampler_create(&dst, &src);
	assert_non_null(rs);
	const float mono[] = {-1.0f, -0.5f, 0.0f, 0.25f, 1.0f};
	const uint8_t *input[MAX_AV_PLANES] = {(const uint8_t *)mono};
	uint8_t *output[MAX_AV_PLANES] = {0};
	uint32_t frames = 0;
	uint64_t offset = UINT64_MAX;
	assert_true(audio_resampler_resample(rs, output, &frames, &offset, input, 5));
	assert_int_equal(frames, 5);
	assert_int_equal(offset, 0);
	for (size_t ch = 0; ch < 2; ch++) {
		assert_non_null(output[ch]);
		for (uint32_t i = 0; i < frames; i++)
			assert_true(((const float *)output[ch])[i] == mono[i]);
	}
	audio_resampler_destroy(rs);
}

static void rate_conversion_delay(void **state)
{
	(void)state;
	const struct resample_info src = {48000, AUDIO_FORMAT_FLOAT, SPEAKERS_MONO};
	const struct resample_info dst = {24000, AUDIO_FORMAT_FLOAT, SPEAKERS_MONO};
	audio_resampler_t *rs = audio_resampler_create(&dst, &src);
	assert_non_null(rs);
	float samples[480];
	for (size_t i = 0; i < 480; i++)
		samples[i] = 0.25f;
	const uint8_t *input[MAX_AV_PLANES] = {(const uint8_t *)samples};
	uint32_t total = 0;
	for (size_t call = 0; call < 100; call++) {
		uint8_t *output[MAX_AV_PLANES] = {0};
		uint32_t frames = 0;
		uint64_t offset = UINT64_MAX;
		assert_true(audio_resampler_resample(rs, output, &frames, &offset, input, 480));
		assert_true(frames > 0 && frames <= 240);
		assert_non_null(output[0]);
		/* The first timestamp has no buffered delay; later packets do.
		 * Bound the filter tail instead of pinning FFmpeg-version rounding. */
		if (call == 0)
			assert_int_equal(offset, 0);
		else
			assert_true(offset > 0 && offset < 2000000);
		for (uint32_t i = 0; i < frames; i++) {
			const float sample = ((const float *)output[0])[i];
			assert_true(sample > 0.249f && sample < 0.251f);
		}
		total += frames;
	}
	assert_true(total >= 24000 - 64 && total < 24000);
	audio_resampler_destroy(rs);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_resampler),        cmocka_unit_test(invalid_sample_rate),
		cmocka_unit_test(unknown_format_is_s16), cmocka_unit_test(planar_to_packed_and_buffer_growth),
		cmocka_unit_test(packed_to_planar),      cmocka_unit_test(mono_upmix),
		cmocka_unit_test(rate_conversion_delay),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
