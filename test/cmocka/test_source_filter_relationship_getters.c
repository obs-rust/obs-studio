/* Actual-library characterization: borrowed filter relationship reads. */
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
	obs_source_t *parent = obs_filter_get_parent(NULL);
	unsigned first_count = log_count;
	int first_level = log_level;
	char first_message[128];
	memcpy(first_message, log_message, sizeof(first_message));
	obs_source_t *target = obs_filter_get_target(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_null(parent);
	assert_null(target);
	assert_int_equal(first_count, 1);
	assert_int_equal(log_count, 2);
	assert_int_equal(first_level, LOG_DEBUG);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(first_message, "obs_filter_get_parent: Null 'filter' parameter");
	assert_string_equal(log_message, "obs_filter_get_target: Null 'filter' parameter");
}
static void independent_borrowed_fields(void **state)
{
	(void)state;
	struct obs_source sources[3] = {0};
	unsigned char snapshot[sizeof(sources)];
	obs_source_t *values[] = {NULL, &sources[0], &sources[1], &sources[2]};
	const enum obs_source_type types[] = {OBS_SOURCE_TYPE_INPUT, OBS_SOURCE_TYPE_FILTER, OBS_SOURCE_TYPE_TRANSITION,
					      OBS_SOURCE_TYPE_SCENE};
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	unsigned before = log_count;
	for (size_t t = 0; t < sizeof(types) / sizeof(types[0]); t++) {
		for (size_t p = 0; p < 4; p++) {
			for (size_t n = 0; n < 4; n++) {
				sources[0].info.type = types[t];
				sources[0].filter_parent = values[p];
				sources[0].filter_target = values[n];
				memcpy(snapshot, sources, sizeof(sources));
				obs_source_t *parent = obs_filter_get_parent(&sources[0]);
				obs_source_t *target = obs_filter_get_target(&sources[0]);
				/* Restore the global handler before any assertion can longjmp. */
				base_set_log_handler(old_handler, old_param);
				assert_ptr_equal(parent, values[p]);
				assert_ptr_equal(target, values[n]);
				assert_memory_equal(snapshot, sources, sizeof(sources));
				assert_int_equal(log_count, before);
				base_set_log_handler(capture_log, NULL);
			}
		}
	}
	base_set_log_handler(old_handler, old_param);
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults),
					   cmocka_unit_test(independent_borrowed_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
