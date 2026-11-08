/* Actual-library characterization; historical log spelling is characterized, not endorsed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "obs-internal.h"

static unsigned log_count;
static int log_level;
static char log_message[256];
#if defined(__GNUC__) || defined(__clang__)
static void capture_log(int level, const char *format, va_list args, void *param) __attribute__((format(printf, 2, 0)));
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_message, sizeof(log_message), format, args);
}

static void characterize(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	assert_int_equal(obs_encoder_get_frame_rate_divisor(NULL), 0);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_encoder_set_frame_rate_divisor: Null 'encoder' parameter");
	const uint32_t values[] = {0, 1, 2, UINT32_C(0x80000000), UINT32_MAX};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		for (int type = OBS_ENCODER_AUDIO; type <= OBS_ENCODER_VIDEO; type++) {
			struct obs_encoder encoder = {0};
			encoder.context.name = "divisor-test";
			encoder.info.type = (enum obs_encoder_type)type;
			encoder.orig_info.type = type == OBS_ENCODER_AUDIO ? OBS_ENCODER_VIDEO : OBS_ENCODER_AUDIO;
			encoder.frame_rate_divisor = values[i];
			encoder.scaled_width = ~values[i];
			struct obs_encoder before;
			memcpy(&before, &encoder, sizeof(before));
			log_count = 0;
			assert_int_equal(obs_encoder_get_frame_rate_divisor(&encoder),
					 type == OBS_ENCODER_VIDEO ? values[i] : 0);
			assert_int_equal(log_count, type == OBS_ENCODER_VIDEO ? 0 : 1);
			if (type == OBS_ENCODER_AUDIO) {
				assert_int_equal(log_level, LOG_WARNING);
				assert_string_equal(
					log_message,
					"obs_encoder_set_frame_rate_divisor: encoder 'divisor-test' is not a video encoder");
			}
			assert_memory_equal(&encoder, &before, sizeof(encoder));
		}
	}
	base_set_log_handler(old_handler, old_param);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(characterize)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
