/* Actual-library characterization: source flags, enabled state and last OBS version. */
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
	uint32_t flags = obs_source_get_flags(NULL);
	unsigned first_count = log_count;
	int first_level = log_level;
	char first_message[128];
	memcpy(first_message, log_message, sizeof(first_message));
	bool enabled = obs_source_enabled(NULL);
	unsigned second_count = log_count;
	int second_level = log_level;
	char second_message[128];
	memcpy(second_message, log_message, sizeof(second_message));
	uint32_t version = obs_source_get_last_obs_version(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_int_equal(flags, 0);
	assert_false(enabled);
	assert_int_equal(version, 0);
	assert_int_equal(first_count, 1);
	assert_int_equal(second_count, 2);
	assert_int_equal(log_count, 3);
	assert_int_equal(first_level, LOG_DEBUG);
	assert_int_equal(second_level, LOG_DEBUG);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(first_message, "obs_source_get_flags: Null 'source' parameter");
	assert_string_equal(second_message, "obs_source_enabled: Null 'source' parameter");
	assert_string_equal(log_message, "obs_source_get_last_obs_version: Null 'source' parameter");
}
static void independent_fields(void **state)
{
	(void)state;
	const uint32_t edges[] = {0, 1, 0x80000000, 0x7fffffff, 0xffffffff, 0xa5a55a5a};
	struct obs_source source = {0};
	unsigned char snapshot[sizeof(source)];
	for (size_t f = 0; f < sizeof(edges) / sizeof(edges[0]); f++) {
		for (size_t v = 0; v < sizeof(edges) / sizeof(edges[0]); v++) {
			for (unsigned e = 0; e < 2; e++) {
				source.flags = edges[f];
				source.last_obs_ver = edges[v];
				source.enabled = e != 0;
				source.temp_removed = e == 0;
				source.default_flags = ~edges[f];
				memcpy(snapshot, &source, sizeof(source));
				for (unsigned repeat = 0; repeat < 2; repeat++) {
					assert_int_equal(obs_source_get_flags(&source), edges[f]);
					assert_int_equal(obs_source_enabled(&source), e);
					assert_int_equal(obs_source_get_last_obs_version(&source), edges[v]);
					assert_memory_equal(snapshot, &source, sizeof(source));
				}
			}
		}
	}
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults), cmocka_unit_test(independent_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
