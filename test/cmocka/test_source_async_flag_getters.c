/* Actual-library characterization: plain borrowed async flag reads. */
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
static void null_defaults(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	bool unbuffered = obs_source_async_unbuffered(NULL);
	unsigned first_count = log_count;
	int first_level = log_level;
	char first_message[128];
	memcpy(first_message, log_message, sizeof(first_message));
	bool decoupled = obs_source_async_decoupled(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_false(unbuffered);
	assert_false(decoupled);
	assert_int_equal(first_count, 1);
	assert_int_equal(log_count, 2);
	assert_int_equal(first_level, LOG_DEBUG);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(first_message, "obs_source_async_unbuffered: Null 'source' parameter");
	assert_string_equal(log_message, "obs_source_async_decoupled: Null 'source' parameter");
}
static void independent_fields(void **state)
{
	(void)state;
	struct obs_source source = {0};
	unsigned char snapshot[sizeof(source)];
	for (int unbuffered = 0; unbuffered < 2; unbuffered++) {
		for (int decoupled = 0; decoupled < 2; decoupled++) {
			source.async_unbuffered = unbuffered != 0;
			source.async_decoupled = decoupled != 0;
			source.async_width = 17;
			source.async_height = 29;
			memcpy(snapshot, &source, sizeof(source));
			assert_int_equal(obs_source_async_unbuffered(&source), unbuffered);
			assert_memory_equal(snapshot, &source, sizeof(source));
			assert_int_equal(obs_source_async_decoupled(&source), decoupled);
			assert_memory_equal(snapshot, &source, sizeof(source));
		}
	}
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults), cmocka_unit_test(independent_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
