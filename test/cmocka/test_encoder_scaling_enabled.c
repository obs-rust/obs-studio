/* Actual-library characterization: scaling depends on either axis, regardless of type. */
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
static char log_message[128];
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

static void null_default(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	assert_false(obs_encoder_scaling_enabled(NULL));
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_encoder_scaling_enabled: Null 'encoder' parameter");
}

static void either_axis_without_mutation(void **state)
{
	(void)state;
	const uint32_t sizes[] = {0, 1, 1920, UINT32_C(0x80000000), UINT32_MAX};
	for (size_t x = 0; x < 5; x++) {
		for (size_t y = 0; y < 5; y++) {
			for (int type = OBS_ENCODER_AUDIO; type <= OBS_ENCODER_VIDEO; type++) {
				struct obs_encoder encoder = {0}, before;
				encoder.info.type = (enum obs_encoder_type)type;
				encoder.scaled_width = sizes[x];
				encoder.scaled_height = sizes[y];
				encoder.gpu_scale_type = OBS_SCALE_DISABLE;
				memcpy(&before, &encoder, sizeof(before));
				/* Characterized, not endorsed: one zero axis and audio encoders are accepted. */
				assert_int_equal(obs_encoder_scaling_enabled(&encoder), x != 0 || y != 0);
				assert_memory_equal(&encoder, &before, sizeof(encoder));
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_default),
					   cmocka_unit_test(either_axis_without_mutation)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
