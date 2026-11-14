/* Characterize the C setters through the exported API and real private layout. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include "obs-internal.h"

static const enum video_colorspace spaces[] = {
	VIDEO_CS_DEFAULT,           VIDEO_CS_601, VIDEO_CS_709, VIDEO_CS_SRGB, VIDEO_CS_2100_PQ, VIDEO_CS_2100_HLG,
	(enum video_colorspace)0x5a};
static const enum video_range_type ranges[] = {VIDEO_RANGE_DEFAULT, VIDEO_RANGE_PARTIAL, VIDEO_RANGE_FULL,
					       (enum video_range_type)0x5a};
/* Out-of-enum values are stored without validation: characterized, not endorsed. */

static void test_null(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(spaces) / sizeof(spaces[0]); i++)
		obs_encoder_set_preferred_color_space(NULL, spaces[i]);
	for (size_t i = 0; i < sizeof(ranges) / sizeof(ranges[0]); i++)
		obs_encoder_set_preferred_range(NULL, ranges[i]);
}

static void test_audio_unchanged(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	struct obs_encoder before;
	encoder.info.type = OBS_ENCODER_AUDIO;
	encoder.orig_info.type = OBS_ENCODER_VIDEO;
	encoder.preferred_format = VIDEO_FORMAT_NV12;
	encoder.preferred_space = VIDEO_CS_2100_HLG;
	encoder.preferred_range = VIDEO_RANGE_FULL;
	memcpy(&before, &encoder, sizeof(before));
	for (size_t i = 0; i < sizeof(spaces) / sizeof(spaces[0]); i++) {
		obs_encoder_set_preferred_color_space(&encoder, spaces[i]);
		assert_memory_equal(&encoder, &before, sizeof(encoder));
	}
	for (size_t i = 0; i < sizeof(ranges) / sizeof(ranges[0]); i++) {
		obs_encoder_set_preferred_range(&encoder, ranges[i]);
		assert_memory_equal(&encoder, &before, sizeof(encoder));
	}
}

static void test_video_color_space_writes_only_its_field(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	struct obs_encoder expected;
	encoder.info.type = OBS_ENCODER_VIDEO;
	encoder.orig_info.type = OBS_ENCODER_AUDIO;
	encoder.preferred_format = VIDEO_FORMAT_NV12;
	/* Cross-product plus repeated writes pins independence and overwrites. */
	for (size_t r = 0; r < sizeof(ranges) / sizeof(ranges[0]); r++) {
		encoder.preferred_range = ranges[r];
		for (size_t i = 0; i < sizeof(spaces) / sizeof(spaces[0]); i++) {
			memcpy(&expected, &encoder, sizeof(expected));
			expected.preferred_space = spaces[i];
			for (size_t repeat = 0; repeat < 2; repeat++) {
				obs_encoder_set_preferred_color_space(&encoder, spaces[i]);
				assert_int_equal(encoder.preferred_space, spaces[i]);
				assert_memory_equal(&encoder, &expected, sizeof(encoder));
			}
		}
	}
}

static void test_video_range_writes_only_its_field(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	struct obs_encoder expected;
	encoder.info.type = OBS_ENCODER_VIDEO;
	encoder.orig_info.type = OBS_ENCODER_AUDIO;
	encoder.preferred_format = VIDEO_FORMAT_NV12;
	for (size_t s = 0; s < sizeof(spaces) / sizeof(spaces[0]); s++) {
		encoder.preferred_space = spaces[s];
		for (size_t i = 0; i < sizeof(ranges) / sizeof(ranges[0]); i++) {
			memcpy(&expected, &encoder, sizeof(expected));
			expected.preferred_range = ranges[i];
			for (size_t repeat = 0; repeat < 2; repeat++) {
				obs_encoder_set_preferred_range(&encoder, ranges[i]);
				assert_int_equal(encoder.preferred_range, ranges[i]);
				assert_memory_equal(&encoder, &expected, sizeof(encoder));
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(test_null), cmocka_unit_test(test_audio_unchanged),
					   cmocka_unit_test(test_video_color_space_writes_only_its_field),
					   cmocka_unit_test(test_video_range_writes_only_its_field)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
