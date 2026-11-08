/* Actual-library characterization: source audio activity. */
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
	bool active = obs_source_audio_active(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_false(active);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, "obs_source_audio_active: Null 'source' parameter");
}
static void independent_fields(void **state)
{
	(void)state;
	struct obs_source source = {0};
	unsigned char snapshot[sizeof(source)];
	for (unsigned bits = 0; bits < 16; bits++) {
		source.audio_active = (bits & 1) != 0;
		source.audio_pending = (bits & 2) != 0;
		source.enabled = (bits & 4) != 0;
		source.removed = (bits & 8) != 0;
		memcpy(snapshot, &source, sizeof(source));
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			assert_int_equal(obs_source_audio_active(&source), (bits & 1) != 0);
			assert_memory_equal(snapshot, &source, sizeof(source));
		}
	}
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults), cmocka_unit_test(independent_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
