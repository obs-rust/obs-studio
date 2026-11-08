/* Actual-library characterization: source UUID and unversioned ID borrowed pointers. */
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
	const char *uuid = obs_source_get_uuid(NULL);
	unsigned first_count = log_count;
	int first_level = log_level;
	char first_message[128];
	memcpy(first_message, log_message, sizeof(first_message));
	const char *id = obs_source_get_unversioned_id(NULL);
	base_set_log_handler(old_handler, old_param);
	assert_null(uuid);
	assert_null(id);
	assert_int_equal(first_count, 1);
	assert_int_equal(log_count, 2);
	assert_int_equal(first_level, LOG_DEBUG);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(first_message, "obs_source_get_uuid: Null 'source' parameter");
	assert_string_equal(log_message, "obs_source_get_unversioned_id: Null 'source' parameter");
}
static void independent_fields(void **state)
{
	(void)state;
	char empty[] = "", uuid[] = "01234567-89ab-cdef-0123-456789abcdef", id[] = "source_v2", bytes[] = "\xff";
	char *values[] = {NULL, empty, uuid, id, bytes};
	struct obs_source source = {0};
	unsigned char snapshot[sizeof(source)];
	for (size_t u = 0; u < 5; u++) {
		for (size_t i = 0; i < 5; i++) {
			source.context.uuid = values[u];
			source.info.unversioned_id = values[i];
			source.context.name = id;
			source.info.id = uuid;
			memcpy(snapshot, &source, sizeof(source));
			for (unsigned repeat = 0; repeat < 2; repeat++) {
				assert_ptr_equal(obs_source_get_uuid(&source), values[u]);
				assert_ptr_equal(obs_source_get_unversioned_id(&source), values[i]);
				assert_memory_equal(snapshot, &source, sizeof(source));
			}
		}
	}
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults), cmocka_unit_test(independent_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
