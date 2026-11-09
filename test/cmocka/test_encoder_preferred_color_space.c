/* Characterize the exported getter against the real private encoder layout. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include "obs-internal.h"

static void test_null(void **state)
{
	(void)state;
	assert_int_equal(obs_encoder_get_preferred_color_space(NULL), VIDEO_CS_DEFAULT);
}

static void test_stored_spaces(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	struct obs_encoder before;
	const enum video_colorspace spaces[] = {VIDEO_CS_DEFAULT, VIDEO_CS_601, VIDEO_CS_709, VIDEO_CS_SRGB,
					       VIDEO_CS_2100_PQ, VIDEO_CS_2100_HLG};
	for (size_t kind = 0; kind < 2; kind++) {
		encoder.info.type = kind ? OBS_ENCODER_VIDEO : OBS_ENCODER_AUDIO;
		encoder.orig_info.type = kind ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		encoder.preferred_format = VIDEO_FORMAT_NV12;
		encoder.preferred_range = VIDEO_RANGE_FULL;
		for (size_t i = 0; i < sizeof(spaces) / sizeof(spaces[0]); i++) {
			encoder.preferred_space = spaces[i];
			memcpy(&before, &encoder, sizeof(before));
			assert_int_equal(obs_encoder_get_preferred_color_space(&encoder),
					 kind ? spaces[i] : VIDEO_CS_DEFAULT);
			assert_memory_equal(&encoder, &before, sizeof(encoder));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(test_null), cmocka_unit_test(test_stored_spaces)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
