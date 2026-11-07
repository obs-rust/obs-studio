/* Characterization of the actual libobs borrowed last-error getter. */
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

static void borrowed_last_error(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	assert_null(obs_encoder_get_last_error(NULL));
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_encoder_get_last_error: Null 'encoder' parameter");
	log_count = 0;
	char empty[] = "", message[] = {'a', (char)0xff, 'b', 0}, other[] = "other";
	char *messages[] = {NULL, empty, message};
	struct obs_encoder second = {0};
	second.last_error_message = other;
	for (size_t i = 0; i < sizeof(messages) / sizeof(messages[0]); i++) {
		struct obs_encoder encoder = {0};
		encoder.last_error_message = messages[i];
		struct obs_encoder before, second_before;
		memcpy(&before, &encoder, sizeof(before));
		memcpy(&second_before, &second, sizeof(second_before));
		assert_ptr_equal(obs_encoder_get_last_error(&encoder), messages[i]);
		assert_ptr_equal(obs_encoder_get_last_error(&second), other);
		assert_memory_equal(&encoder, &before, sizeof(encoder));
		assert_memory_equal(&second, &second_before, sizeof(second));
		assert_int_equal(log_count, 0);
	}
	base_set_log_handler(old_handler, old_param);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(borrowed_last_error)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
