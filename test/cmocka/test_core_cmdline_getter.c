/* Public getter characterization against the actual linked libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <cmocka.h>
#include "obs.h"
#include "util/bmem.h"

static void command_line_snapshot(void **state)
{
	(void)state;
	struct obs_cmdline_args initial = obs_get_cmdline_args();
	assert_int_equal(initial.argc, 0);
	assert_null(initial.argv);
	assert_int_equal(obs_get_cmdline_args().argc, 0);
	assert_null(obs_get_cmdline_args().argv);

	char program[] = "obs";
	const char invalid[] = {(char)0xff, (char)0x80, 0};
	const char *input[] = {program, "", "--profile=\xc3\xa9", invalid};
	obs_set_cmdline_args(4, input);
	struct obs_cmdline_args first = obs_get_cmdline_args();
	assert_int_equal(first.argc, 4);
	assert_non_null(first.argv);
	assert_ptr_not_equal(first.argv, input);
	for (int i = 0; i < first.argc; i++) {
		assert_ptr_not_equal(first.argv[i], input[i]);
		assert_string_equal(first.argv[i], input[i]);
	}
	assert_null(first.argv[4]);
	program[0] = 'X';
	first.argc = 99;
	const char *ignored[] = {"ignored"};
	obs_set_cmdline_args(1, ignored);
	struct obs_cmdline_args again = obs_get_cmdline_args();
	assert_int_equal(again.argc, 4);
	assert_ptr_equal(again.argv, first.argv);
	assert_string_equal(again.argv[0], "obs");
	assert_string_equal(again.argv[1], "");
	assert_string_equal(again.argv[2], "--profile=\xc3\xa9");
	assert_string_equal(again.argv[3], invalid);
	assert_null(again.argv[4]);
	/* obs_shutdown frees exactly this allocation; no core was started here. */
	bfree(again.argv);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(command_line_snapshot)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
