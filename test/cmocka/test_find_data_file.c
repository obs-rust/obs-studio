/* Direct characterization of obs_find_data_file before extraction (#5641).
 * No core/display is needed. The extra-path list is process-global; each
 * fixture owns its paths and files. macOS bundle lookup may return a path
 * without checking existence (characterized, not endorsed). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include <obs.h>
#include <util/base.h>
#include <util/bmem.h>
#include <util/platform.h>

#if defined(_MSC_VER)
#pragma warning(disable : 4996)
#else
#pragma GCC diagnostic ignored "-Wdeprecated-declarations"
#endif

#define ROOT "test_find_data_file_tmp/"
#define FIRST ROOT "first/"
#define SECOND ROOT "second/"
#define PROBE "obs_find_data_file_probe.txt"
#define MISSING "obs_find_data_file_missing.txt"

static log_handler_t old_log;
static void *old_log_param;
static unsigned errors;
static char last_error[512];

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	if (level == LOG_ERROR) {
		++errors;
		vsnprintf(last_error, sizeof(last_error), format, args);
	}
}

static int setup(void **state)
{
	(void)state;
	base_get_log_handler(&old_log, &old_log_param);
	base_set_log_handler(capture_log, NULL);
	errors = 0;
	last_error[0] = 0;
	os_mkdir(ROOT);
	os_mkdir(FIRST);
	os_mkdir(SECOND);
	assert_true(os_quick_write_utf8_file(FIRST PROBE, "first", 5, false));
	assert_true(os_quick_write_utf8_file(SECOND PROBE, "second", 6, false));
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	while (obs_remove_data_path(FIRST)) {
	}
	while (obs_remove_data_path(SECOND)) {
	}
	while (obs_remove_data_path(ROOT "absent/")) {
	}
	while (obs_remove_data_path(FIRST "obs_find_data_file_")) {
	}
	os_unlink(FIRST PROBE);
	os_unlink(SECOND PROBE);
	os_unlink(SECOND "default.effect");
	os_rmdir(FIRST);
	os_rmdir(SECOND);
	os_rmdir(ROOT);
	base_set_log_handler(old_log, old_log_param);
	return 0;
}

static void require_extra_lookup(void)
{
	char *path = obs_find_data_file(MISSING);
	if (path) {
		assert_false(os_file_exists(path));
		bfree(path);
		skip(); /* The bundle's unconditional path wins before extra paths. */
	}
	errors = 0;
}

static void ordered_first_hit_and_owned_results(void **state)
{
	(void)state;
	require_extra_lookup();
	obs_add_data_path(ROOT "absent/");
	obs_add_data_path(FIRST);
	obs_add_data_path(SECOND);
	long before = bnum_allocs();
	char *a = obs_find_data_file(PROBE);
	char *b = obs_find_data_file(PROBE);
	assert_non_null(a);
	assert_non_null(b);
	assert_ptr_not_equal(a, b);
	assert_string_equal(a, FIRST PROBE);
	assert_string_equal(b, FIRST PROBE);
	a[0] = 'X';
	assert_string_equal(b, FIRST PROBE);
	bfree(a);
	bfree(b);
	assert_int_equal(bnum_allocs(), before);
	assert_int_equal(errors, 0);
	assert_true(obs_remove_data_path(FIRST));
	a = obs_find_data_file(PROBE);
	assert_string_equal(a, SECOND PROBE);
	bfree(a);
}

static void misses_log_once_and_free_temporary_path(void **state)
{
	(void)state;
	require_extra_lookup();
	obs_add_data_path(FIRST);
	obs_add_data_path(SECOND);
	long before = bnum_allocs();
	assert_null(obs_find_data_file(MISSING));
	assert_int_equal(errors, 1);
	assert_string_equal(last_error, "Failed to find file '" MISSING "' in libobs data directory");
	assert_int_equal(bnum_allocs(), before);
}

static void path_is_a_literal_prefix(void **state)
{
	(void)state;
	require_extra_lookup();
	/* No slash is inserted between the registered path and requested name. */
	obs_add_data_path(FIRST "obs_find_data_file_");
	char *path = obs_find_data_file("probe.txt");
	assert_string_equal(path, FIRST PROBE);
	bfree(path);
	assert_int_equal(errors, 0);
}

static void builtin_precedes_extra_paths(void **state)
{
	(void)state;
	char *builtin = obs_find_data_file("default.effect");
	if (!builtin)
		skip(); /* Some standalone harnesses do not install libobs resources. */
	assert_true(os_quick_write_utf8_file(SECOND "default.effect", "shadow", 6, false));
	obs_add_data_path(SECOND);
	errors = 0;
	char *path = obs_find_data_file("default.effect");
	assert_non_null(path);
	assert_string_equal(path, builtin);
	assert_string_not_equal(path, SECOND "default.effect");
	bfree(path);
	bfree(builtin);
	assert_int_equal(errors, 0);
}

static void missing_builtin_path_or_failure(void **state)
{
	(void)state;
	long before = bnum_allocs();
	char *path = obs_find_data_file(MISSING);
	if (path) {
		/* macOS framework Resources path need not exist. It still wins
		 * over an extra directory, and each returned path is caller-owned. */
		assert_false(os_file_exists(path));
		obs_add_data_path(SECOND);
		char *again = obs_find_data_file(MISSING);
		assert_non_null(again);
		assert_ptr_not_equal(path, again);
		assert_string_equal(path, again);
		bfree(again);
		bfree(path);
		assert_true(obs_remove_data_path(SECOND));
		assert_int_equal(errors, 0);
	} else {
		assert_int_equal(errors, 1);
		assert_string_equal(last_error, "Failed to find file '" MISSING "' in libobs data directory");
	}
	/* Adding/removing paths retains list capacity, so only the lookup itself
	 * is allocation-neutral in the missing-file branch. */
	if (errors)
		assert_int_equal(bnum_allocs(), before);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(ordered_first_hit_and_owned_results, setup, teardown),
		cmocka_unit_test_setup_teardown(misses_log_once_and_free_temporary_path, setup, teardown),
		cmocka_unit_test_setup_teardown(path_is_a_literal_prefix, setup, teardown),
		cmocka_unit_test_setup_teardown(builtin_precedes_extra_paths, setup, teardown),
		cmocka_unit_test_setup_teardown(missing_builtin_path_or_failure, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
