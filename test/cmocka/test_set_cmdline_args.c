/* C-only characterization before extracting obs_set_cmdline_args (Refs
 * #5468).
 *
 * test_core_cmdline_getter pins the copy and the once-only rule, and
 * test_core_setup_rules_steps the sizes and the block layout of a first call
 * with arguments. This test pins the rest of the public behavior, counting
 * bmem allocations:
 *
 * - the copy is one allocation: the table of argc + 1 pointers, then each
 *   string right after the previous one's terminator, argv[argc] NULL;
 * - a first call with argc 0 stores an empty table (argv[0] NULL) and a later
 *   call with arguments still copies them: a zero count counts as unset. The
 *   later copy does not free the empty table (characterized, not endorsed;
 *   the sanitized steps test leaves this case out for that reason);
 * - obs_shutdown() frees the copy, and startup + shutdown leave no other
 *   allocation behind.
 *
 * The command line is process-global and set once, so the cases run in this
 * order in one process, each checking the state it starts from. The frontend
 * sets the command line before obs_startup(), and so does this test. After
 * obs_shutdown() the stored argv points at freed memory, so nothing reads it
 * and no second core is started (its shutdown would free the copy again).
 *
 * obs_startup() needs a display on Linux (run under xvfb-run); without one
 * set, the shutdown case skips. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/bmem.h>

/* ------------------------------------------------------------------------ */
/* Cases                                                                     */

static void zero_count_stores_empty_table(void **state)
{
	(void)state;
	struct obs_cmdline_args initial = obs_get_cmdline_args();
	assert_int_equal(initial.argc, 0);
	assert_null(initial.argv);

	const char *const none[] = {NULL};
	long allocs = bnum_allocs();
	obs_set_cmdline_args(0, none);
	assert_int_equal(bnum_allocs(), allocs + 1);

	struct obs_cmdline_args args = obs_get_cmdline_args();
	assert_int_equal(args.argc, 0);
	assert_non_null(args.argv);
	assert_ptr_not_equal(args.argv, none);
	assert_null(args.argv[0]);
}

static void later_call_copies_into_one_block(void **state)
{
	(void)state;
	/* A zero count counts as unset: a later call still copies. */
	assert_int_equal(obs_get_cmdline_args().argc, 0);

	const char *const input[] = {"obs", "--startreplaybuffer", "", "--scene=\xc3\xa9 x"};
	long allocs = bnum_allocs();
	obs_set_cmdline_args(4, input);
	/* One new block; the empty table of the zero-count call stays allocated. */
	assert_int_equal(bnum_allocs(), allocs + 1);

	struct obs_cmdline_args args = obs_get_cmdline_args();
	assert_int_equal(args.argc, 4);
	assert_non_null(args.argv);

	/* The strings start right after the table of argc + 1 pointers and each
	 * follows the previous one's terminator. */
	const char *next = (const char *)(args.argv + 5);
	for (int i = 0; i < 4; i++) {
		assert_ptr_not_equal(args.argv[i], input[i]);
		assert_ptr_equal(args.argv[i], next);
		assert_string_equal(args.argv[i], input[i]);
		next += strlen(input[i]) + 1;
	}
	assert_null(args.argv[4]);
}

static void shutdown_frees_the_copy(void **state)
{
	(void)state;
	if (obs_get_cmdline_args().argc == 0) {
		/* Run alone: set a command line first. */
		const char *const input[] = {"obs", "--verbose"};
		obs_set_cmdline_args(2, input);
	}
	assert_int_not_equal(obs_get_cmdline_args().argc, 0);
	assert_non_null(obs_get_cmdline_args().argv);

	long allocs = bnum_allocs();
	if (!obs_startup("en-US", NULL, NULL)) {
		assert_null(getenv("DISPLAY"));
		assert_null(getenv("WAYLAND_DISPLAY"));
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		skip();
	}
	assert_true(obs_initialized());
	obs_shutdown();
	assert_false(obs_initialized());

	/* The copy was freed with the core, and nothing else was left. */
	assert_int_equal(bnum_allocs(), allocs - 1);
	/* The count stays, so a later call is still ignored: no new copy. */
	assert_int_not_equal(obs_get_cmdline_args().argc, 0);
	const char *const later[] = {"later"};
	allocs = bnum_allocs();
	obs_set_cmdline_args(1, later);
	assert_int_equal(bnum_allocs(), allocs);
}

int main(void)
{
	/* In this order: each case starts from the state the previous one left. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(zero_count_stores_empty_table),
		cmocka_unit_test(later_call_copies_into_one_block),
		cmocka_unit_test(shutdown_frees_the_copy),
	};
	return cmocka_run_group_tests_name("test_set_cmdline_args", tests, NULL, NULL);
}
