#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>
#include <sys/stat.h>
#include <cmocka.h>

#include <util/platform.h>
#include <util/bmem.h>

/*
 * Characterization tests for the file helpers in libobs/util/platform-file.c
 * (#262 slice 3) that test_platform.c leaves uncovered: os_wfopen, os_fread_mbs
 * called directly, os_fgetsize restoring the position, 64-bit seek offsets,
 * os_stat, zero-length writes and NULL paths. They pin the current C behavior
 * for the Rust port; cases marked "characterized, not endorsed" are odd but
 * real.
 *
 * Everything lives under a temp dir relative to cwd, with '/' separators.
 */

#define ROOT "test_platform_file.tmp"

#define P_WIDE ROOT "/wide_\xC3\xBC.txt"
#define P_MBS ROOT "/mbs.txt"
#define P_SIZE ROOT "/size.txt"
#define P_SEEK ROOT "/seek.txt"
#define P_STAT ROOT "/stat.txt"
#define P_EMPTY_UTF8 ROOT "/empty_utf8.txt"
#define P_EMPTY_MBS ROOT "/empty_mbs.txt"
#define P_MISSING ROOT "/missing.txt"
#define P_SAFE ROOT "/safe.txt"
#define P_SAFE_TMP ROOT "/safe.txt.tmp"

static const char *const file_paths[] = {
	P_WIDE, P_MBS, P_SIZE, P_SEEK, P_STAT, P_EMPTY_UTF8, P_EMPTY_MBS, P_MISSING, P_SAFE, P_SAFE_TMP,
};

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);

	os_mkdirs(ROOT);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);

	for (size_t i = 0; i < sizeof(file_paths) / sizeof(file_paths[0]); i++)
		os_unlink(file_paths[i]);
	os_rmdir(ROOT);
	return 0;
}

static bool write_str(const char *path, const char *str)
{
	return os_quick_write_utf8_file(path, str, strlen(str), false);
}

static void test_fopen_null_path(void **state)
{
	UNUSED_PARAMETER(state);

	assert_null(os_fopen(NULL, "rb"));
	assert_null(os_wfopen(NULL, "rb"));
}

static void test_wfopen_non_ascii_path(void **state)
{
	UNUSED_PARAMETER(state);

	assert_true(write_str(P_WIDE, "wide"));

	wchar_t *wpath = NULL;
	assert_true(os_utf8_to_wcs_ptr(P_WIDE, 0, &wpath) > 0);

	FILE *f = os_wfopen(wpath, "rb");
	bfree(wpath);
	assert_non_null(f);

	char buf[8] = {0};
	assert_int_equal(fread(buf, 1, sizeof(buf) - 1, f), 4);
	assert_string_equal(buf, "wide");
	fclose(f);
}

static void test_wfopen_missing(void **state)
{
	UNUSED_PARAMETER(state);

	wchar_t *wpath = NULL;
	assert_true(os_utf8_to_wcs_ptr(P_MISSING, 0, &wpath) > 0);
	assert_null(os_wfopen(wpath, "rb"));
	bfree(wpath);
}

static void test_fread_mbs_ascii(void **state)
{
	UNUSED_PARAMETER(state);

	assert_true(os_quick_write_mbs_file(P_MBS, "hello", 5));

	FILE *f = os_fopen(P_MBS, "rb");
	assert_non_null(f);

	/* reads from the start even when the position is elsewhere */
	assert_int_equal(os_fseeki64(f, 2, SEEK_SET), 0);

	char *s = NULL;
	assert_int_equal(os_fread_mbs(f, &s), 5);
	assert_non_null(s);
	assert_string_equal(s, "hello");
	bfree(s);

	/* characterized, not endorsed: the position is left at the end */
	assert_int_equal(os_ftelli64(f), 5);
	fclose(f);
}

static void test_fread_mbs_empty(void **state)
{
	UNUSED_PARAMETER(state);

	assert_true(os_quick_write_mbs_file(P_EMPTY_MBS, "", 0));
	assert_int_equal(os_get_file_size(P_EMPTY_MBS), 0);

	FILE *f = os_fopen(P_EMPTY_MBS, "rb");
	assert_non_null(f);

	char *s = (char *)"untouched";
	assert_int_equal(os_fread_mbs(f, &s), 0);
	assert_null(s);
	fclose(f);

	assert_null(os_quick_read_mbs_file(P_EMPTY_MBS));
	assert_null(os_quick_read_mbs_file(P_MISSING));
}

static void test_fgetsize_restores_position(void **state)
{
	UNUSED_PARAMETER(state);

	assert_true(write_str(P_SIZE, "0123456789"));

	FILE *f = os_fopen(P_SIZE, "rb");
	assert_non_null(f);
	assert_int_equal(os_fseeki64(f, 4, SEEK_SET), 0);

	assert_int_equal(os_fgetsize(f), 10);
	assert_int_equal(os_ftelli64(f), 4);

	char c = 0;
	assert_int_equal(fread(&c, 1, 1, f), 1);
	assert_int_equal(c, '4');
	fclose(f);
}

static void test_seek_tell_beyond_4gib(void **state)
{
	UNUSED_PARAMETER(state);

	assert_true(write_str(P_SEEK, "x"));

	FILE *f = os_fopen(P_SEEK, "rb");
	assert_non_null(f);

	/* seeking past the end without writing does not grow the file */
	const int64_t beyond = INT64_C(5) * 1024 * 1024 * 1024 + 3;
	assert_int_equal(os_fseeki64(f, beyond, SEEK_SET), 0);
	assert_true(os_ftelli64(f) == beyond);

	assert_int_equal(os_fseeki64(f, -2, SEEK_CUR), 0);
	assert_true(os_ftelli64(f) == beyond - 2);

	assert_int_equal(os_fseeki64(f, 0, SEEK_END), 0);
	assert_true(os_ftelli64(f) == 1);
	fclose(f);

	assert_int_equal(os_get_file_size(P_SEEK), 1);
}

static void test_stat(void **state)
{
	UNUSED_PARAMETER(state);

	assert_true(write_str(P_STAT, "abcdef"));

	struct stat st;
	memset(&st, 0, sizeof(st));
	assert_int_equal(os_stat(P_STAT, &st), 0);
	assert_int_equal(st.st_size, 6);
	assert_true((st.st_mode & S_IFMT) == S_IFREG);

	assert_int_not_equal(os_stat(P_MISSING, &st), 0);

	memset(&st, 0, sizeof(st));
	assert_int_equal(os_stat(ROOT, &st), 0);
	assert_true((st.st_mode & S_IFMT) == S_IFDIR);
}

static void test_utf8_write_empty(void **state)
{
	UNUSED_PARAMETER(state);

	/* len 0 writes nothing; str is not read */
	assert_true(os_quick_write_utf8_file(P_EMPTY_UTF8, NULL, 0, false));
	assert_int_equal(os_get_file_size(P_EMPTY_UTF8), 0);

	assert_true(os_quick_write_utf8_file(P_EMPTY_UTF8, NULL, 0, true));
	assert_int_equal(os_get_file_size(P_EMPTY_UTF8), 3);

	/* characterized, not endorsed: a BOM-only file reads as NULL */
	assert_null(os_quick_read_utf8_file(P_EMPTY_UTF8));
}

static void test_utf8_write_safe_ext_without_dot(void **state)
{
	UNUSED_PARAMETER(state);

	os_unlink(P_SAFE);
	assert_true(os_quick_write_utf8_file_safe(P_SAFE, "v1", 2, false, "tmp", NULL));

	char *s = os_quick_read_utf8_file(P_SAFE);
	assert_non_null(s);
	assert_string_equal(s, "v1");
	bfree(s);

	/* the temp file was "<path>.tmp" and has been renamed over the target */
	assert_false(os_file_exists(P_SAFE_TMP));

	assert_false(os_quick_write_utf8_file_safe(P_SAFE, "v2", 2, false, "", NULL));
	assert_false(os_quick_write_utf8_file_safe(P_SAFE, "v2", 2, false, NULL, NULL));
}

static void test_get_file_size_null(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(os_get_file_size(NULL), -1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_fopen_null_path),       cmocka_unit_test(test_wfopen_non_ascii_path),
		cmocka_unit_test(test_wfopen_missing),        cmocka_unit_test(test_fread_mbs_ascii),
		cmocka_unit_test(test_fread_mbs_empty),       cmocka_unit_test(test_fgetsize_restores_position),
		cmocka_unit_test(test_seek_tell_beyond_4gib), cmocka_unit_test(test_stat),
		cmocka_unit_test(test_utf8_write_empty),      cmocka_unit_test(test_utf8_write_safe_ext_without_dot),
		cmocka_unit_test(test_get_file_size_null),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
