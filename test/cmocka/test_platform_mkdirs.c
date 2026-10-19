#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include <util/platform.h>

/*
 * Characterization tests for os_mkdirs in libobs/util/platform-mkdirs.c
 * (#262 slice 4), which test_platform.c only uses for setup: missing parents,
 * paths that already exist, '\' separators, trailing and doubled slashes and
 * a file in the way. They pin the current C behavior for the Rust port; cases
 * marked "characterized, not endorsed" are odd but real.
 *
 * Everything lives under a temp dir relative to cwd.
 */

#define ROOT "test_platform_mkdirs.tmp"

#define D_FILE ROOT "/file"

/* removed deepest first */
static const char *const dir_paths[] = {
	ROOT "/n1/n2/n3", ROOT "/n1/n2", ROOT "/n1",       ROOT "/single",     ROOT "/b1/b2/b3",
	ROOT "/b1/b2",    ROOT "/b1",    ROOT "/m1/m2/m3", ROOT "/m1/m2",      ROOT "/m1",
	ROOT "/t1/t2",    ROOT "/t1",    ROOT "/t3",       ROOT "/d1/d2",      ROOT "/d1",
	ROOT "/p1/p2/p3", ROOT "/p1/p2", ROOT "/p1",       ROOT "/\xC3\xBC/x", ROOT "/\xC3\xBC",
};

static bool is_dir(const char *path)
{
	os_dir_t *dir = os_opendir(path);
	if (!dir)
		return false;
	os_closedir(dir);
	return true;
}

static void clean(void)
{
	for (size_t i = 0; i < sizeof(dir_paths) / sizeof(dir_paths[0]); i++)
		os_rmdir(dir_paths[i]);
	os_unlink(D_FILE);
	os_rmdir(ROOT);
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	clean();
	return os_mkdir(ROOT) == MKDIR_SUCCESS ? 0 : -1;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);

	clean();
	return 0;
}

static void test_mkdirs_creates_missing_parents(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(os_mkdirs(ROOT "/n1/n2/n3"), MKDIR_SUCCESS);
	assert_true(is_dir(ROOT "/n1"));
	assert_true(is_dir(ROOT "/n1/n2"));
	assert_true(is_dir(ROOT "/n1/n2/n3"));

	assert_int_equal(os_mkdirs(ROOT "/n1/n2/n3"), MKDIR_EXISTS);
	assert_int_equal(os_mkdirs(ROOT "/n1"), MKDIR_EXISTS);
}

static void test_mkdirs_single_component(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(os_mkdirs(ROOT "/single"), MKDIR_SUCCESS);
	assert_true(is_dir(ROOT "/single"));
	assert_int_equal(os_mkdirs(ROOT "/single"), MKDIR_EXISTS);
	assert_int_equal(os_mkdirs(ROOT), MKDIR_EXISTS);
}

static void test_mkdirs_some_parents_exist(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(os_mkdir(ROOT "/p1"), MKDIR_SUCCESS);
	assert_int_equal(os_mkdirs(ROOT "/p1/p2/p3"), MKDIR_SUCCESS);
	assert_true(is_dir(ROOT "/p1/p2/p3"));
}

static void test_mkdirs_backslashes(void **state)
{
	UNUSED_PARAMETER(state);

	char path[] = ROOT "\\b1\\b2\\b3";

	/* '\' becomes '/' on every OS, so this nests three directories */
	assert_int_equal(os_mkdirs(path), MKDIR_SUCCESS);
	assert_true(is_dir(ROOT "/b1/b2/b3"));

	/* the caller's string is left alone */
	assert_string_equal(path, ROOT "\\b1\\b2\\b3");

	assert_int_equal(os_mkdirs(ROOT "\\b1\\b2"), MKDIR_EXISTS);
}

static void test_mkdirs_mixed_separators(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(os_mkdirs(ROOT "/m1\\m2/m3"), MKDIR_SUCCESS);
	assert_true(is_dir(ROOT "/m1/m2/m3"));
}

static void test_mkdirs_trailing_slash(void **state)
{
	UNUSED_PARAMETER(state);

	/* parent exists: created on the first try */
	assert_int_equal(os_mkdirs(ROOT "/t3/"), MKDIR_SUCCESS);
	assert_true(is_dir(ROOT "/t3"));
	assert_int_equal(os_mkdirs(ROOT "/t3/"), MKDIR_EXISTS);

	/* characterized, not endorsed: with a missing parent, the recursion
	 * creates "t1/t2" from "t1/t2/" minus its last slash, then the final
	 * os_mkdir of "t1/t2/" finds it there, so a call that created two
	 * directories reports MKDIR_EXISTS */
	assert_int_equal(os_mkdirs(ROOT "/t1/t2/"), MKDIR_EXISTS);
	assert_true(is_dir(ROOT "/t1/t2"));
}

static void test_mkdirs_doubled_slash(void **state)
{
	UNUSED_PARAMETER(state);

	/* the parent tried is "d1/" (only the last slash is cut) */
	assert_int_equal(os_mkdirs(ROOT "/d1//d2"), MKDIR_SUCCESS);
	assert_true(is_dir(ROOT "/d1/d2"));
}

static void test_mkdirs_non_ascii(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(os_mkdirs(ROOT "/\xC3\xBC/x"), MKDIR_SUCCESS);
	assert_true(is_dir(ROOT "/\xC3\xBC/x"));
}

static void test_mkdirs_file_in_the_way(void **state)
{
	UNUSED_PARAMETER(state);

	assert_true(os_quick_write_utf8_file(D_FILE, "x", 1, false));

	/* characterized, not endorsed: a file at the path counts as existing */
	assert_int_equal(os_mkdirs(D_FILE), MKDIR_EXISTS);
	assert_false(is_dir(D_FILE));

	/* a file as a parent fails: the recursion stops at the file, which
	 * counts as existing, and the retry of the full path fails */
	assert_int_equal(os_mkdirs(D_FILE "/sub"), MKDIR_ERROR);
	assert_int_equal(os_mkdirs(D_FILE "/sub/deeper"), MKDIR_ERROR);
	assert_false(is_dir(D_FILE "/sub"));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_mkdirs_creates_missing_parents), cmocka_unit_test(test_mkdirs_single_component),
		cmocka_unit_test(test_mkdirs_some_parents_exist),      cmocka_unit_test(test_mkdirs_backslashes),
		cmocka_unit_test(test_mkdirs_mixed_separators),        cmocka_unit_test(test_mkdirs_trailing_slash),
		cmocka_unit_test(test_mkdirs_doubled_slash),           cmocka_unit_test(test_mkdirs_non_ascii),
		cmocka_unit_test(test_mkdirs_file_in_the_way),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
