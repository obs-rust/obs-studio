/* Direct characterization, before the Rust swap. Real-device UTF-8 loading,
 * construction and cache lifetime remain covered by test_effect_destroy. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "effect-file-fixture.h"

static void validation(void **state)
{
	(void)state;
	ef_reset(false, true, true);
	ef_file = NULL;
	char *error = (char *)"unchanged";
	ef_error = &error;
	assert_null(ef_from_file(NULL, &error));
	assert_string_equal(ef_log, "gs_effect_create_from_file: called while not in a graphics context");
	assert_int_equal(ef_level, LOG_DEBUG);
	assert_string_equal(ef_events, "L");
	assert_string_equal(error, "unchanged");
	ef_reset(true, true, true);
	assert_null(ef_from_file(NULL, &error));
	assert_string_equal(ef_log, "gs_effect_create_from_file: Null 'file' parameter");
	assert_int_equal(ef_level, LOG_DEBUG);
	assert_string_equal(ef_events, "L");
	assert_string_equal(error, "unchanged");
}
static void cache(void **state)
{
	(void)state;
	ef_reset(true, false, false);
	ef_effects[0].effect_path = (char *)"CASE.effect";
	ef_effects[0].next = &ef_effects[1];
	ef_effects[1].effect_path = (char *)"case.effect";
	ef_effects[1].next = &ef_effects[2];
	ef_effects[2].effect_path = (char *)"case.effect";
	ef_graphics.first_effect = &ef_effects[0];
	char *error = (char *)"unchanged";
	assert_ptr_equal(ef_from_file("case.effect", &error), &ef_effects[1]);
	assert_ptr_equal(ef_from_file("CASE.effect", NULL), &ef_effects[0]);
	assert_string_equal(error, "unchanged");
	assert_string_equal(ef_events, "");
}
static void reads_and_constructor(void **state)
{
	(void)state;
	for (int read_ok = 0; read_ok < 2; read_ok++) {
		for (int create_ok = 0; create_ok < 2; create_ok++) {
			for (int with_error = 0; with_error < 2; with_error++) {
				ef_reset(true, read_ok != 0, create_ok != 0);
				ef_file = "missing.effect";
				char *error = (char *)"unchanged";
				ef_error = with_error ? &error : NULL;
				assert_ptr_equal(ef_from_file(ef_file, ef_error),
						 read_ok && create_ok ? &ef_effects[7] : NULL);
				assert_string_equal(ef_events, read_ok ? "RCF" : "RL");
				assert_true(ef_args_ok);
				if (!read_ok) {
					assert_string_equal(error, "unchanged");
					assert_int_equal(ef_level, LOG_ERROR);
					assert_string_equal(ef_log, "Could not load effect file 'missing.effect'");
				} else {
					assert_true(ef_error_forwarded);
					if (with_error)
						assert_ptr_equal(error, create_ok ? NULL : ef_text);
				}
			}
		}
	}
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(validation), cmocka_unit_test(cache),
					   cmocka_unit_test(reads_and_constructor)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
