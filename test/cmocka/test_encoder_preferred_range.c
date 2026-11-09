/* Characterize the actual exported getter over the private real C layout. */
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
	assert_int_equal(obs_encoder_get_preferred_range(NULL), VIDEO_RANGE_DEFAULT);
}

static void test_stored_ranges(void **state)
{
	(void)state;
	struct obs_encoder encoder = {0};
	struct obs_encoder before;
	const enum video_range_type ranges[] = {VIDEO_RANGE_DEFAULT, VIDEO_RANGE_PARTIAL, VIDEO_RANGE_FULL};
	for (size_t kind = 0; kind < 2; kind++) {
		encoder.info.type = kind ? OBS_ENCODER_VIDEO : OBS_ENCODER_AUDIO;
		encoder.orig_info.type = kind ? OBS_ENCODER_AUDIO : OBS_ENCODER_VIDEO;
		encoder.preferred_format = VIDEO_FORMAT_NV12;
		encoder.preferred_space = VIDEO_CS_709;
		for (size_t i = 0; i < sizeof(ranges) / sizeof(ranges[0]); i++) {
			encoder.preferred_range = ranges[i];
			memcpy(&before, &encoder, sizeof(before));
			assert_int_equal(obs_encoder_get_preferred_range(&encoder),
					 kind ? ranges[i] : VIDEO_RANGE_DEFAULT);
			assert_memory_equal(&encoder, &before, sizeof(encoder));
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(test_null), cmocka_unit_test(test_stored_ranges)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
