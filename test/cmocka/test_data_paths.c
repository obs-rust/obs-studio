/* Characterization of obs_add_data_path() and obs_remove_data_path() (#5355),
 * the deprecated public wrappers around the process-global list of extra data
 * directories that obs_find_data_file() searches after the libobs data
 * directory. It pins the CURRENT behaviour before the Rust swap (#5397),
 * through the public API only, so the same test runs on the hybrid libobs and
 * libobs-c-reference.
 *
 * What it pins:
 *
 *   - the list needs no core: adding and removing work before obs_startup(),
 *     and a path added before startup survives startup and shutdown;
 *   - obs_add_data_path() copies the caller's bytes: changing or freeing the
 *     caller's buffer afterwards changes nothing;
 *   - duplicates are kept: each add needs its own remove;
 *   - obs_remove_data_path() removes only the first equal entry, keeps the
 *     others in their order, and returns false when nothing matches;
 *   - matching is exact and case-sensitive, with no path normalisation (a
 *     trailing slash matters);
 *   - NULL and "" are the same path: either one adds an entry that the other
 *     removes;
 *   - obs_find_data_file() searches the added paths in insertion order, so
 *     the order left after a removal is observable.
 *
 * The existing test_core_setup_rules_steps and
 * test_obs_core_info_characterization cases are left as they are; this file
 * adds the ownership, NULL/empty, order-after-removal and lifecycle cases.
 *
 * obs_startup() needs a display on Linux, so the lifecycle case skips when it
 * fails without one. On macOS obs_find_data_file() returns the libobs
 * framework's Resources path for any name without checking that it exists,
 * so the order cases skip there. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <util/bmem.h>
#include <util/platform.h>

/* The deprecated calls are still exported; their behavior is pinned before
 * the port like every other export. */
#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

#define DIR_A "test_data_paths_a/"
#define DIR_B "test_data_paths_b/"
#define PROBE "test_data_paths_probe.txt"
#define MISSING "test_data_paths_missing.txt"

static void test_add_remove_without_core(void **state)
{
	(void)state;
	assert_false(obs_initialized());

	assert_false(obs_remove_data_path("test_data_paths_never/"));
	obs_add_data_path("test_data_paths_x/");
	assert_true(obs_remove_data_path("test_data_paths_x/"));
	assert_false(obs_remove_data_path("test_data_paths_x/"));
}

static void test_add_copies_caller_bytes(void **state)
{
	(void)state;

	char *buf = bstrdup("test_data_paths_copy/");
	obs_add_data_path(buf);
	buf[0] = 'X';
	assert_false(obs_remove_data_path(buf));
	bfree(buf);

	char stack[32];
	snprintf(stack, sizeof(stack), "%s", "test_data_paths_stack/");
	obs_add_data_path(stack);
	memset(stack, 0, sizeof(stack));

	assert_true(obs_remove_data_path("test_data_paths_copy/"));
	assert_true(obs_remove_data_path("test_data_paths_stack/"));
	assert_false(obs_remove_data_path("test_data_paths_copy/"));
	assert_false(obs_remove_data_path("test_data_paths_stack/"));
}

static void test_duplicates_need_one_remove_each(void **state)
{
	(void)state;

	for (int i = 0; i < 3; i++)
		obs_add_data_path("test_data_paths_dup/");
	for (int i = 0; i < 3; i++)
		assert_true(obs_remove_data_path("test_data_paths_dup/"));
	assert_false(obs_remove_data_path("test_data_paths_dup/"));
}

static void test_match_is_exact(void **state)
{
	(void)state;

	obs_add_data_path("test_data_paths_Case/");
	assert_false(obs_remove_data_path("test_data_paths_case/"));
	assert_false(obs_remove_data_path("TEST_DATA_PATHS_CASE/"));
	assert_false(obs_remove_data_path("test_data_paths_Case"));
	assert_false(obs_remove_data_path("test_data_paths_Case//"));
	assert_false(obs_remove_data_path("./test_data_paths_Case/"));
	assert_false(obs_remove_data_path("test_data_paths_Cas"));
	assert_true(obs_remove_data_path("test_data_paths_Case/"));
	assert_false(obs_remove_data_path("test_data_paths_Case/"));
}

static void test_null_and_empty_are_the_same_path(void **state)
{
	(void)state;

	assert_false(obs_remove_data_path(NULL));
	assert_false(obs_remove_data_path(""));

	obs_add_data_path(NULL);
	assert_true(obs_remove_data_path(""));
	assert_false(obs_remove_data_path(NULL));

	obs_add_data_path("");
	assert_true(obs_remove_data_path(NULL));
	assert_false(obs_remove_data_path(""));

	obs_add_data_path(NULL);
	obs_add_data_path("");
	assert_true(obs_remove_data_path(NULL));
	assert_true(obs_remove_data_path(NULL));
	assert_false(obs_remove_data_path(""));

	/* An empty entry does not match a real path, nor the reverse. */
	obs_add_data_path("");
	assert_false(obs_remove_data_path("test_data_paths_x/"));
	obs_add_data_path("test_data_paths_x/");
	assert_true(obs_remove_data_path(""));
	assert_false(obs_remove_data_path(NULL));
	assert_true(obs_remove_data_path("test_data_paths_x/"));
}

static bool write_probe(const char *dir, const char *content)
{
	char file[128];
	snprintf(file, sizeof(file), "%s%s", dir, PROBE);
	os_mkdir(dir);
	return os_quick_write_utf8_file(file, content, strlen(content), false);
}

static void remove_probe(const char *dir)
{
	char file[128];
	snprintf(file, sizeof(file), "%s%s", dir, PROBE);
	os_unlink(file);
	os_rmdir(dir);
}

/* Returns which directory obs_find_data_file() picked for PROBE: 'a', 'b',
 * or 0 when it found nothing. */
static char found_in(void)
{
	char *found = obs_find_data_file(PROBE);
	if (!found)
		return 0;
	char which = 0;
	if (strcmp(found, DIR_A PROBE) == 0)
		which = 'a';
	else if (strcmp(found, DIR_B PROBE) == 0)
		which = 'b';
	bfree(found);
	assert_true(which != 0);
	return which;
}

static void skip_if_lookup_never_misses(void)
{
	char *missing = obs_find_data_file(MISSING);
	if (missing) {
		/* macOS: see the header comment. */
		assert_false(os_file_exists(missing));
		bfree(missing);
		skip();
	}
}

static void test_remove_keeps_order_of_the_rest(void **state)
{
	(void)state;
	skip_if_lookup_never_misses();

	assert_true(write_probe(DIR_A, "a"));
	assert_true(write_probe(DIR_B, "b"));

	assert_int_equal(found_in(), 0);

	/* A, B, A: the first A wins. */
	obs_add_data_path(DIR_A);
	obs_add_data_path(DIR_B);
	obs_add_data_path(DIR_A);
	assert_int_equal(found_in(), 'a');

	/* Removing A drops the first A only, leaving B, A: B now wins. If the
	 * last equal entry were removed instead, A would still be first. */
	assert_true(obs_remove_data_path(DIR_A));
	assert_int_equal(found_in(), 'b');

	/* Removing B leaves the second A. */
	assert_true(obs_remove_data_path(DIR_B));
	assert_int_equal(found_in(), 'a');

	/* Adding B again appends it after A. */
	obs_add_data_path(DIR_B);
	assert_int_equal(found_in(), 'a');

	assert_true(obs_remove_data_path(DIR_A));
	assert_int_equal(found_in(), 'b');
	assert_true(obs_remove_data_path(DIR_B));
	assert_int_equal(found_in(), 0);
	assert_false(obs_remove_data_path(DIR_A));
	assert_false(obs_remove_data_path(DIR_B));

	remove_probe(DIR_A);
	remove_probe(DIR_B);
}

static void test_paths_outlive_startup_and_shutdown(void **state)
{
	(void)state;
	assert_false(obs_initialized());

	obs_add_data_path("test_data_paths_before/");
	obs_add_data_path("test_data_paths_before/");

	if (!obs_startup("en-US", NULL, NULL)) {
		bool display = getenv("DISPLAY") || getenv("WAYLAND_DISPLAY");
		assert_true(obs_remove_data_path("test_data_paths_before/"));
		assert_true(obs_remove_data_path("test_data_paths_before/"));
		if (display)
			fprintf(stderr, "obs_startup failed with a display set\n");
		assert_false(display);
		skip();
	}

	/* Startup neither clears nor duplicates the list. */
	assert_true(obs_remove_data_path("test_data_paths_before/"));
	obs_add_data_path("test_data_paths_during/");
	obs_add_data_path("test_data_paths_during/");

	obs_shutdown();
	assert_false(obs_initialized());

	/* Shutdown leaves both the entry added before startup and the ones
	 * added while the core ran. */
	assert_true(obs_remove_data_path("test_data_paths_before/"));
	assert_false(obs_remove_data_path("test_data_paths_before/"));
	assert_true(obs_remove_data_path("test_data_paths_during/"));
	assert_true(obs_remove_data_path("test_data_paths_during/"));
	assert_false(obs_remove_data_path("test_data_paths_during/"));

	/* And the list still works after shutdown. */
	obs_add_data_path("test_data_paths_after/");
	assert_true(obs_remove_data_path("test_data_paths_after/"));
	assert_false(obs_remove_data_path("test_data_paths_after/"));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_add_remove_without_core),
		cmocka_unit_test(test_add_copies_caller_bytes),
		cmocka_unit_test(test_duplicates_need_one_remove_each),
		cmocka_unit_test(test_match_is_exact),
		cmocka_unit_test(test_null_and_empty_are_the_same_path),
		cmocka_unit_test(test_remove_keeps_order_of_the_rest),
		/* Last: it starts and stops the core. */
		cmocka_unit_test(test_paths_outlive_startup_and_shutdown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
