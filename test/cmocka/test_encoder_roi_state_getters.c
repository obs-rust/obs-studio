/* Actual-library characterization; synthetic private state is never destroyed. */
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

static void roi_state_getters(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	assert_false(obs_encoder_has_roi(NULL));
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_encoder_has_roi: Null 'encoder' parameter");
	log_count = 0;
	assert_int_equal(obs_encoder_get_roi_increment(NULL), 0);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_encoder_get_roi_increment: Null 'encoder' parameter");
	log_count = 0;
	const size_t counts[] = {0, 1, 2, SIZE_MAX};
	const uint32_t increments[] = {0, 1, UINT32_C(0x80000001), UINT32_MAX};
	struct obs_encoder second = {0};
	second.roi.num = 1;
	second.roi_increment = 19;
	for (size_t i = 0; i < sizeof(counts) / sizeof(counts[0]); i++) {
		for (size_t j = 0; j < sizeof(increments) / sizeof(increments[0]); j++) {
			struct obs_encoder encoder = {0};
			/* Characterized, not endorsed: count is read without a lock or dereferencing the array. */
			encoder.roi.num = counts[i];
			encoder.roi_increment = increments[j];
			struct obs_encoder before, second_before;
			memcpy(&before, &encoder, sizeof(before));
			memcpy(&second_before, &second, sizeof(second_before));
			assert_int_equal(obs_encoder_has_roi(&encoder), counts[i] > 0);
			assert_int_equal(obs_encoder_get_roi_increment(&encoder), increments[j]);
			assert_true(obs_encoder_has_roi(&second));
			assert_int_equal(obs_encoder_get_roi_increment(&second), 19);
			assert_memory_equal(&encoder, &before, sizeof(encoder));
			assert_memory_equal(&second, &second_before, sizeof(second));
			assert_int_equal(log_count, 0);
		}
	}
	base_set_log_handler(old_handler, old_param);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(roi_state_getters)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
