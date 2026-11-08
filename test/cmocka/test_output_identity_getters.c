/* Characterization of the actual libobs instance metadata getters. */
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

static void expect_log(const char *function)
{
	char expected[128];
	snprintf(expected, sizeof(expected), "%s: Null 'output' parameter", function);
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_message, expected);
	log_count = 0;
}

static void null_defaults(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	assert_null(obs_output_get_id(NULL));
	expect_log("obs_output_get_id");
	assert_null(obs_output_get_type_data(NULL));
	expect_log("obs_output_get_type_data");
	base_set_log_handler(old_handler, old_param);
}

static void exact_independent_metadata(void **state)
{
	(void)state;
	char id[] = "current-id", empty[] = "", bytes[] = {(char)0xff, 'x', 0};
	int data = 17;
	const char *strings[] = {NULL, empty, id, bytes};
	void *pointers[] = {NULL, &data};
	for (size_t i = 0; i < sizeof(strings) / sizeof(strings[0]); i++) {
		for (size_t j = 0; j < sizeof(pointers) / sizeof(pointers[0]); j++) {
			struct obs_output output = {0};
			output.info.id = strings[i];
			output.info.type_data = pointers[j];
			output.context.name = "different-context-name";
			output.context.data = &output;
			struct obs_output before;
			memcpy(&before, &output, sizeof(before));
			assert_ptr_equal(obs_output_get_id(&output), strings[i]);
			assert_ptr_equal(obs_output_get_type_data(&output), pointers[j]);
			assert_memory_equal(&output, &before, sizeof(output));
		}
	}
	assert_int_equal(data, 17);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(null_defaults),
					   cmocka_unit_test(exact_independent_metadata)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
