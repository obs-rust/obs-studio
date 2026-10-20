#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <string.h>
#include <stdlib.h>
#include <stdio.h>
#include <locale.h>
#include <wchar.h>
#include <cmocka.h>

#include <util/platform.h>
#include <util/bmem.h>
#include <obs.h>

#ifndef _WIN32
#include <sys/types.h>
#include <sys/wait.h>
#include <signal.h>
#include <unistd.h>
#endif

/*
 * Characterization tests for the file and string helpers in
 * libobs/util/platform.c (+ platform-nix.c / platform-windows.c).
 * Path helpers and os_generate_formatted_filename are covered by
 * test_os_path.c and test_formatted_filename.c; the latter replaces
 * obs_get_video_info with a fixture, so the call before obs_startup, with the
 * real one, is here.
 *
 * Everything lives under a temp dir relative to cwd, with '/' separators.
 */

#define ROOT "test_platform.tmp"

#define P_UTF8_PLAIN ROOT "/utf8_plain.txt"
#define P_UTF8_BOM ROOT "/utf8_bom.txt"
#define P_UTF8_EMPTY ROOT "/utf8_empty.txt"
#define P_UTF8_BOMONLY ROOT "/utf8_bomonly.txt"
#define P_FREAD ROOT "/fread.txt"
#define P_SAFE ROOT "/safe.txt"
#define P_SAFE_TMP ROOT "/safe.txt.tmp"
#define P_SAFE_BAK ROOT "/safe.txt.bak"
#define P_MBS ROOT "/mbs.txt"
#define P_MISSING ROOT "/missing.txt"
#define P_EXISTS ROOT "/exists.txt"
#define P_REN_A ROOT "/ren_a.txt"
#define P_REN_B ROOT "/ren_b.txt"
#define P_SR_TARGET ROOT "/sr_target.txt"
#define P_SR_FROM ROOT "/sr_from.txt"
#define P_SR_BAK ROOT "/sr_bak.txt"
#define P_SEEK ROOT "/seek.txt"
#define P_LIST_1 ROOT "/list/l1.txt"
#define P_LIST_2 ROOT "/list/l2.txt"

#define D_LIST ROOT "/list"
#define D_LIST_SUB ROOT "/list/sub"
#define D_MK ROOT "/mk1"
#define D_NEST_A ROOT "/a"
#define D_NEST_B ROOT "/a/b"
#define D_NEST_C ROOT "/a/b/c"

static const char *const file_paths[] = {
	P_UTF8_PLAIN, P_UTF8_BOM, P_UTF8_EMPTY, P_UTF8_BOMONLY, P_FREAD,  P_SAFE,  P_SAFE_TMP,
	P_SAFE_BAK,   P_MBS,      P_MISSING,    P_EXISTS,       P_REN_A,  P_REN_B, P_SR_TARGET,
	P_SR_FROM,    P_SR_BAK,   P_SEEK,       P_LIST_1,       P_LIST_2,
};

/* deepest first */
static const char *const dir_paths[] = {
	D_NEST_C, D_NEST_B, D_NEST_A, D_LIST_SUB, D_LIST, D_MK,
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
	for (size_t i = 0; i < sizeof(dir_paths) / sizeof(dir_paths[0]); i++)
		os_rmdir(dir_paths[i]);
	os_rmdir(ROOT);
	return 0;
}

static void clean(const char *path)
{
	os_unlink(path);
}

static bool write_str(const char *path, const char *str)
{
	return os_quick_write_utf8_file(path, str, strlen(str), false);
}

/* reads a file expecting non-NULL content and compares it */
static void assert_file_content(const char *path, const char *expected)
{
	char *s = os_quick_read_utf8_file(path);
	assert_non_null(s);
	assert_string_equal(s, expected);
	bfree(s);
}

static void test_utf8_write_no_marker(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_UTF8_PLAIN);

	assert_true(os_quick_write_utf8_file(P_UTF8_PLAIN, "hello", 5, false));
	assert_int_equal(os_get_file_size(P_UTF8_PLAIN), 5);
	assert_file_content(P_UTF8_PLAIN, "hello");
}

static void test_utf8_write_marker_adds_bom_and_read_strips(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_UTF8_BOM);

	assert_true(os_quick_write_utf8_file(P_UTF8_BOM, "hello", 5, true));
	/* BOM adds 3 bytes */
	assert_int_equal(os_get_file_size(P_UTF8_BOM), 8);
	/* os_fread_utf8 skips a leading EF BB BF */
	assert_file_content(P_UTF8_BOM, "hello");
}

static void test_utf8_read_empty_and_bom_only_return_null(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_UTF8_EMPTY);
	clean(P_UTF8_BOMONLY);

	/* characterized, not endorsed: an empty file reads back as NULL, not "" */
	assert_true(os_quick_write_utf8_file(P_UTF8_EMPTY, "", 0, false));
	assert_int_equal(os_get_file_size(P_UTF8_EMPTY), 0);
	assert_null(os_quick_read_utf8_file(P_UTF8_EMPTY));

	/* a BOM with no payload is also NULL */
	assert_true(os_quick_write_utf8_file(P_UTF8_BOMONLY, "", 0, true));
	assert_int_equal(os_get_file_size(P_UTF8_BOMONLY), 3);
	assert_null(os_quick_read_utf8_file(P_UTF8_BOMONLY));
}

static void test_utf8_read_missing_returns_null(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_MISSING);
	assert_null(os_quick_read_utf8_file(P_MISSING));
	assert_null(os_quick_read_mbs_file(P_MISSING));
}

static void test_fread_utf8_returns_zero_length(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_FREAD);
	assert_true(write_str(P_FREAD, "abc"));

	FILE *f = os_fopen(P_FREAD, "rb");
	assert_non_null(f);

	char *str = NULL;
	size_t len = os_fread_utf8(f, &str);
	fclose(f);

	/* characterized, not endorsed: the string is filled in but the
	 * returned length is always 0 */
	assert_int_equal(len, 0);
	assert_non_null(str);
	assert_string_equal(str, "abc");
	bfree(str);
}

static void test_utf8_write_to_bad_path_fails(void **state)
{
	UNUSED_PARAMETER(state);

	assert_false(os_quick_write_utf8_file(ROOT "/no_such_dir/x.txt", "a", 1, false));
}

static void test_utf8_write_safe_with_backup(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_SAFE);
	clean(P_SAFE_TMP);
	clean(P_SAFE_BAK);

	/* no existing target: the temp file becomes the target, no backup */
	assert_true(os_quick_write_utf8_file_safe(P_SAFE, "one", 3, false, "tmp", "bak"));
	assert_true(os_file_exists(P_SAFE));
	assert_false(os_file_exists(P_SAFE_TMP));
	assert_false(os_file_exists(P_SAFE_BAK));
	assert_file_content(P_SAFE, "one");

	/* existing target: old content moves to the backup; leading '.' in
	 * the extension is optional */
	assert_true(os_quick_write_utf8_file_safe(P_SAFE, "two", 3, false, ".tmp", ".bak"));
	assert_true(os_file_exists(P_SAFE));
	assert_false(os_file_exists(P_SAFE_TMP));
	assert_true(os_file_exists(P_SAFE_BAK));
	assert_file_content(P_SAFE, "two");
	assert_file_content(P_SAFE_BAK, "one");
}

static void test_utf8_write_safe_without_backup(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_SAFE);
	clean(P_SAFE_TMP);
	clean(P_SAFE_BAK);

	assert_true(write_str(P_SAFE, "old"));
	assert_true(os_quick_write_utf8_file_safe(P_SAFE, "new", 3, false, "tmp", NULL));
	assert_file_content(P_SAFE, "new");
	assert_false(os_file_exists(P_SAFE_TMP));
	assert_false(os_file_exists(P_SAFE_BAK));

	/* empty backup extension is treated like none */
	assert_true(os_quick_write_utf8_file_safe(P_SAFE, "newer", 5, false, "tmp", ""));
	assert_file_content(P_SAFE, "newer");
	assert_false(os_file_exists(P_SAFE_BAK));
}

static void test_utf8_write_safe_invalid_temp_ext(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_SAFE);

	assert_false(os_quick_write_utf8_file_safe(P_SAFE, "x", 1, false, NULL, "bak"));
	assert_false(os_quick_write_utf8_file_safe(P_SAFE, "x", 1, false, "", "bak"));
	assert_false(os_file_exists(P_SAFE));
}

static void test_mbs_roundtrip_ascii(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_MBS);

	assert_true(os_quick_write_mbs_file(P_MBS, "hello world", 11));
	assert_int_equal(os_get_file_size(P_MBS), 11);

	char *s = os_quick_read_mbs_file(P_MBS);
	assert_non_null(s);
	assert_string_equal(s, "hello world");
	bfree(s);

	/* len 0 means strlen */
	assert_true(os_quick_write_mbs_file(P_MBS, "abc", 0));
	assert_int_equal(os_get_file_size(P_MBS), 3);
}

static void test_get_file_size_missing(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_MISSING);
	assert_true(os_get_file_size(P_MISSING) == -1);
}

static void test_file_exists(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_EXISTS);
	assert_false(os_file_exists(P_EXISTS));
	assert_true(write_str(P_EXISTS, "x"));
	assert_true(os_file_exists(P_EXISTS));
	assert_int_equal(os_unlink(P_EXISTS), 0);
	assert_false(os_file_exists(P_EXISTS));
}

static void test_unlink_missing_fails(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_MISSING);
	assert_int_equal(os_unlink(P_MISSING), -1);
}

static void test_mkdir_success_then_exists(void **state)
{
	UNUSED_PARAMETER(state);

	os_rmdir(D_MK);

	assert_int_equal(MKDIR_SUCCESS, 0);
	assert_int_equal(MKDIR_EXISTS, 1);
	assert_int_equal(MKDIR_ERROR, -1);

	assert_int_equal(os_mkdir(D_MK), MKDIR_SUCCESS);
	assert_int_equal(os_mkdir(D_MK), MKDIR_EXISTS);
	assert_int_equal(os_rmdir(D_MK), 0);
	assert_false(os_file_exists(D_MK));
}

static void test_mkdir_missing_parent_is_error(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(os_mkdir(ROOT "/no_parent/child"), MKDIR_ERROR);
}

static void test_mkdirs_nested(void **state)
{
	UNUSED_PARAMETER(state);

	os_rmdir(D_NEST_C);
	os_rmdir(D_NEST_B);
	os_rmdir(D_NEST_A);

	assert_int_equal(os_mkdirs(D_NEST_C), MKDIR_SUCCESS);
	assert_true(os_file_exists(D_NEST_A));
	assert_true(os_file_exists(D_NEST_B));
	assert_true(os_file_exists(D_NEST_C));

	/* already there */
	assert_int_equal(os_mkdirs(D_NEST_C), MKDIR_EXISTS);

	/* backslashes are normalized to '/' */
	assert_int_equal(os_mkdirs(ROOT "\\a\\b\\c"), MKDIR_EXISTS);
}

static void test_rename(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_REN_A);
	clean(P_REN_B);

	assert_true(write_str(P_REN_A, "payload"));
	assert_int_equal(os_rename(P_REN_A, P_REN_B), 0);
	assert_false(os_file_exists(P_REN_A));
	assert_true(os_file_exists(P_REN_B));
	assert_file_content(P_REN_B, "payload");

	/* missing source fails */
	assert_int_equal(os_rename(P_REN_A, P_REN_B), -1);
}

static void test_safe_replace_with_backup(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_SR_TARGET);
	clean(P_SR_FROM);
	clean(P_SR_BAK);

	assert_true(write_str(P_SR_TARGET, "old"));
	assert_true(write_str(P_SR_FROM, "new"));

	assert_int_equal(os_safe_replace(P_SR_TARGET, P_SR_FROM, P_SR_BAK), 0);
	assert_file_content(P_SR_TARGET, "new");
	assert_file_content(P_SR_BAK, "old");
	assert_false(os_file_exists(P_SR_FROM));
}

static void test_safe_replace_without_backup(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_SR_TARGET);
	clean(P_SR_FROM);
	clean(P_SR_BAK);

	assert_true(write_str(P_SR_TARGET, "old"));
	assert_true(write_str(P_SR_FROM, "new"));

	assert_int_equal(os_safe_replace(P_SR_TARGET, P_SR_FROM, NULL), 0);
	assert_file_content(P_SR_TARGET, "new");
	assert_false(os_file_exists(P_SR_FROM));
	assert_false(os_file_exists(P_SR_BAK));
}

static void test_fopen_seek_tell(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_SEEK);
	clean(P_MISSING);
	assert_true(write_str(P_SEEK, "0123456789"));

	assert_null(os_fopen(NULL, "rb"));
	assert_null(os_fopen(P_MISSING, "rb"));

	FILE *f = os_fopen(P_SEEK, "rb");
	assert_non_null(f);

	assert_true(os_ftelli64(f) == 0);
	assert_int_equal(os_fseeki64(f, 4, SEEK_SET), 0);
	assert_true(os_ftelli64(f) == 4);
	assert_int_equal(fgetc(f), '4');
	assert_true(os_ftelli64(f) == 5);

	assert_int_equal(os_fseeki64(f, -2, SEEK_END), 0);
	assert_true(os_ftelli64(f) == 8);

	/* os_fgetsize reports the size and restores the position */
	assert_true(os_fgetsize(f) == 10);
	assert_true(os_ftelli64(f) == 8);

	fclose(f);
}

static int cmp_str(const void *a, const void *b)
{
	return strcmp(*(const char *const *)a, *(const char *const *)b);
}

static void test_opendir_readdir_closedir(void **state)
{
	UNUSED_PARAMETER(state);

	clean(P_LIST_1);
	clean(P_LIST_2);
	os_rmdir(D_LIST_SUB);
	os_rmdir(D_LIST);

	assert_int_equal(os_mkdir(D_LIST), MKDIR_SUCCESS);
	assert_int_equal(os_mkdir(D_LIST_SUB), MKDIR_SUCCESS);
	assert_true(write_str(P_LIST_1, "1"));
	assert_true(write_str(P_LIST_2, "2"));

	/* the POSIX implementation keeps the path pointer, so pass a literal */
	os_dir_t *dir = os_opendir(D_LIST);
	assert_non_null(dir);

	char names[8][256];
	bool is_dir[8];
	size_t count = 0;
	struct os_dirent *ent;

	while ((ent = os_readdir(dir)) != NULL) {
		if (strcmp(ent->d_name, ".") == 0 || strcmp(ent->d_name, "..") == 0)
			continue;
		assert_true(count < 8);
		strcpy(names[count], ent->d_name);
		is_dir[count] = ent->directory;
		count++;
	}
	os_closedir(dir);

	assert_int_equal(count, 3);

	/* sort indices by name */
	const char *sorted[3];
	for (size_t i = 0; i < 3; i++)
		sorted[i] = names[i];
	qsort((void *)sorted, 3, sizeof(sorted[0]), cmp_str);

	assert_string_equal(sorted[0], "l1.txt");
	assert_string_equal(sorted[1], "l2.txt");
	assert_string_equal(sorted[2], "sub");

	for (size_t i = 0; i < 3; i++) {
		bool expect_dir = strcmp(names[i], "sub") == 0;
		assert_true(is_dir[i] == expect_dir);
	}

	/* missing directory */
	assert_null(os_opendir(ROOT "/no_such_dir"));
	/* closing/reading NULL is tolerated */
	assert_null(os_readdir(NULL));
	os_closedir(NULL);
}

static void test_get_abs_path_ptr_dot(void **state)
{
	UNUSED_PARAMETER(state);

	char *apath = os_get_abs_path_ptr(".");
	assert_non_null(apath);
	assert_true(strlen(apath) > 0);
#ifdef _WIN32
	/* drive letter form, e.g. C:/... or C:\... */
	assert_true(apath[1] == ':');
#else
	assert_true(apath[0] == '/');
#endif
	bfree(apath);
}

static void test_utf8_wcs_roundtrip_ascii(void **state)
{
	UNUSED_PARAMETER(state);

	wchar_t *w = NULL;
	char *s = NULL;

	assert_int_equal(os_utf8_to_wcs_ptr("hello", 0, &w), 5);
	assert_non_null(w);
	assert_true(wcscmp(w, L"hello") == 0);

	assert_int_equal(os_wcs_to_utf8_ptr(w, 0, &s), 5);
	assert_non_null(s);
	assert_string_equal(s, "hello");

	bfree(w);
	bfree(s);

	/* explicit length truncates */
	w = NULL;
	assert_int_equal(os_utf8_to_wcs_ptr("hello", 3, &w), 3);
	assert_true(wcscmp(w, L"hel") == 0);
	bfree(w);
}

static void test_utf8_wcs_roundtrip_multibyte(void **state)
{
	UNUSED_PARAMETER(state);

	/* "caf" + U+00E9 (2 UTF-8 bytes) + U+20AC (3 UTF-8 bytes) */
	const char *utf8 = "caf\xC3\xA9\xE2\x82\xAC";
	wchar_t *w = NULL;
	char *s = NULL;

	assert_int_equal(os_utf8_to_wcs_ptr(utf8, 0, &w), 5);
	assert_non_null(w);
	assert_int_equal((unsigned)w[3], 0x00E9);
	assert_int_equal((unsigned)w[4], 0x20AC);
	assert_int_equal((unsigned)w[5], 0);

	assert_int_equal(os_wcs_to_utf8_ptr(w, 0, &s), 8);
	assert_non_null(s);
	assert_string_equal(s, utf8);

	bfree(w);
	bfree(s);
}

static void test_utf8_wcs_null_input(void **state)
{
	UNUSED_PARAMETER(state);

	wchar_t *w = (wchar_t *)0x1;
	char *s = (char *)0x1;

	assert_int_equal(os_utf8_to_wcs_ptr(NULL, 0, &w), 0);
	assert_null(w);
	assert_int_equal(os_wcs_to_utf8_ptr(NULL, 0, &s), 0);
	assert_null(s);
}

static void test_mbs_to_utf8_ptr_ascii(void **state)
{
	UNUSED_PARAMETER(state);

	char *s = NULL;

	assert_int_equal(os_mbs_to_utf8_ptr("hello", 0, &s), 5);
	assert_non_null(s);
	assert_string_equal(s, "hello");
	bfree(s);

	s = (char *)0x1;
	assert_int_equal(os_mbs_to_utf8_ptr(NULL, 0, &s), 0);
	assert_null(s);
}

/* Switches LC_CTYPE to a UTF-8 locale; false where none is installed. */
static bool set_utf8_ctype(void)
{
	static const char *const names[] = {"C.UTF-8", "en_US.UTF-8", ".UTF8", "UTF-8"};
	for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); i++) {
		if (setlocale(LC_CTYPE, names[i]))
			return true;
	}
	return false;
}

/*
 * mbstowcs and wcstombs return (size_t)-1 for text the current locale
 * cannot convert. That is a failed conversion: 0 and an empty string. It
 * used to be taken as a length, so the _ptr functions asked for
 * bmalloc((SIZE_MAX + 1) * size), which is bmalloc(0) and crashes, and the
 * plain ones wrote a NUL at dst[SIZE_MAX].
 */
static void test_mbs_unconvertible_is_empty(void **state)
{
	UNUSED_PARAMETER(state);

	char *saved = bstrdup(setlocale(LC_CTYPE, NULL));
	char *s = NULL;
	wchar_t *w = NULL;
	char buf[8];
	wchar_t wbuf[8];

	/* U+4E2D has no byte in the C locale, on every platform */
	const wchar_t wide[] = {L'a', 0x4e2d, 0};
	assert_non_null(setlocale(LC_CTYPE, "C"));

	assert_int_equal(os_wcs_to_mbs(wide, 0, NULL, 0), 0);
	assert_int_equal(os_wcs_to_mbs_ptr(wide, 0, &s), 0);
	assert_non_null(s);
	assert_string_equal(s, "");
	bfree(s);

	memset(buf, 'x', sizeof(buf));
	assert_int_equal(os_wcs_to_mbs(wide, 0, buf, sizeof(buf)), 0);
	assert_string_equal(buf, "");

	assert_int_equal(os_utf8_to_mbs_ptr("a\xe4\xb8\xad", 0, &s), 0);
	assert_non_null(s);
	assert_string_equal(s, "");
	bfree(s);

	/* invalid UTF-8 in a UTF-8 locale */
	if (set_utf8_ctype()) {
		const char *bad = "bad \xff byte";

		assert_int_equal(os_mbs_to_wcs(bad, 0, NULL, 0), 0);
		assert_int_equal(os_mbs_to_wcs_ptr(bad, 0, &w), 0);
		assert_non_null(w);
		assert_int_equal(w[0], 0);
		bfree(w);

		wmemset(wbuf, L'x', 8);
		assert_int_equal(os_mbs_to_wcs(bad, 0, wbuf, 8), 0);
		assert_int_equal(wbuf[0], 0);

		assert_int_equal(os_mbs_to_utf8_ptr(bad, 0, &s), 0);
		assert_non_null(s);
		assert_string_equal(s, "");
		bfree(s);
	} else {
		print_message("no UTF-8 locale: invalid-UTF-8 cases skipped\n");
	}

	setlocale(LC_CTYPE, saved);
	bfree(saved);
}

/*
 * With a buffer too small for the text, mbstowcs and wcstombs fill it
 * completely; the result is cut to dst_size - 1 elements and terminated,
 * instead of the NUL landing one past the end.
 */
static void test_mbs_buffer_too_small_is_cut(void **state)
{
	UNUSED_PARAMETER(state);

	char *saved = bstrdup(setlocale(LC_CTYPE, NULL));
	assert_non_null(setlocale(LC_CTYPE, "C"));

	struct {
		wchar_t w[4];
		wchar_t guard;
	} wide;
	wide.guard = L'G';
	assert_int_equal(os_mbs_to_wcs("abcdef", 0, wide.w, 4), 3);
	assert_true(wcscmp(wide.w, L"abc") == 0);
	assert_int_equal(wide.guard, L'G');

	struct {
		char s[4];
		char guard;
	} narrow;
	narrow.guard = 'G';
	assert_int_equal(os_wcs_to_mbs(L"abcdef", 0, narrow.s, 4), 3);
	assert_string_equal(narrow.s, "abc");
	assert_int_equal(narrow.guard, 'G');

	/* text that fits exactly is unchanged */
	assert_int_equal(os_mbs_to_wcs("abc", 0, wide.w, 4), 3);
	assert_true(wcscmp(wide.w, L"abc") == 0);
	assert_int_equal(os_wcs_to_mbs(L"abc", 0, narrow.s, 4), 3);
	assert_string_equal(narrow.s, "abc");

	setlocale(LC_CTYPE, saved);
	bfree(saved);
}

static void test_strtod(void **state)
{
	UNUSED_PARAMETER(state);

	assert_true(os_strtod("3.25") == 3.25);
	assert_true(os_strtod("-1.5") == -1.5);
	assert_true(os_strtod("1e2") == 100.0);
	assert_true(os_strtod("42") == 42.0);
	/* unparsable input yields 0 */
	assert_true(os_strtod("abc") == 0.0);
}

static void test_dtostr(void **state)
{
	UNUSED_PARAMETER(state);

	char buf[64];

	assert_int_equal(os_dtostr(1.5, buf, sizeof(buf)), 3);
	assert_string_equal(buf, "1.5");

	/* ".0" is appended when there is no dot or exponent */
	assert_int_equal(os_dtostr(0.0, buf, sizeof(buf)), 3);
	assert_string_equal(buf, "0.0");

	assert_int_equal(os_dtostr(-2.25, buf, sizeof(buf)), 5);
	assert_string_equal(buf, "-2.25");

	/* too small a buffer */
	assert_int_equal(os_dtostr(1.5, buf, 3), -1);
	/* no room for the appended ".0" */
	assert_int_equal(os_dtostr(0.0, buf, 3), -1);
}

/* Before obs_startup there is no core, so there is no video: the real
 * obs_get_video_info returns false, and a file name with no video specifier
 * formats normally (#1509). Exit codes: 0 ok, 1 video info, 2 name. */
static int formatted_filename_before_startup(void)
{
	struct obs_video_info ovi;
	if (obs_get_video_info(&ovi))
		return 1;

	char *name = os_generate_formatted_filename("mkv", true, "before startup");
	int ret = name && strcmp(name, "before startup.mkv") == 0 ? 0 : 2;
	if (ret)
		fprintf(stderr, "got \"%s\"\n", name ? name : "(null)");
	bfree(name);
	return ret;
}

static void test_formatted_filename_before_startup(void **state)
{
	UNUSED_PARAMETER(state);

	assert_false(obs_initialized());
#ifdef _WIN32
	assert_int_equal(formatted_filename_before_startup(), 0);
#else
	/* in a child, so a crash fails this test only */
	pid_t pid = fork();
	assert_true(pid >= 0);
	if (pid == 0) {
		/* cmocka catches these; in the child, let a crash end it */
		signal(SIGSEGV, SIG_DFL);
		signal(SIGBUS, SIG_DFL);
		signal(SIGILL, SIG_DFL);
		signal(SIGFPE, SIG_DFL);
		_exit(formatted_filename_before_startup());
	}

	int status = 0;
	assert_int_equal(waitpid(pid, &status, 0), pid);
	if (WIFSIGNALED(status))
		fprintf(stderr, "child killed by signal %d\n", WTERMSIG(status));
	assert_true(WIFEXITED(status));
	assert_int_equal(WEXITSTATUS(status), 0);
#endif
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_utf8_write_no_marker),
		cmocka_unit_test(test_utf8_write_marker_adds_bom_and_read_strips),
		cmocka_unit_test(test_utf8_read_empty_and_bom_only_return_null),
		cmocka_unit_test(test_utf8_read_missing_returns_null),
		cmocka_unit_test(test_fread_utf8_returns_zero_length),
		cmocka_unit_test(test_utf8_write_to_bad_path_fails),
		cmocka_unit_test(test_utf8_write_safe_with_backup),
		cmocka_unit_test(test_utf8_write_safe_without_backup),
		cmocka_unit_test(test_utf8_write_safe_invalid_temp_ext),
		cmocka_unit_test(test_mbs_roundtrip_ascii),
		cmocka_unit_test(test_get_file_size_missing),
		cmocka_unit_test(test_file_exists),
		cmocka_unit_test(test_unlink_missing_fails),
		cmocka_unit_test(test_mkdir_success_then_exists),
		cmocka_unit_test(test_mkdir_missing_parent_is_error),
		cmocka_unit_test(test_mkdirs_nested),
		cmocka_unit_test(test_rename),
		cmocka_unit_test(test_safe_replace_with_backup),
		cmocka_unit_test(test_safe_replace_without_backup),
		cmocka_unit_test(test_fopen_seek_tell),
		cmocka_unit_test(test_opendir_readdir_closedir),
		cmocka_unit_test(test_get_abs_path_ptr_dot),
		cmocka_unit_test(test_utf8_wcs_roundtrip_ascii),
		cmocka_unit_test(test_utf8_wcs_roundtrip_multibyte),
		cmocka_unit_test(test_utf8_wcs_null_input),
		cmocka_unit_test(test_mbs_to_utf8_ptr_ascii),
		cmocka_unit_test(test_mbs_unconvertible_is_empty),
		cmocka_unit_test(test_mbs_buffer_too_small_is_cut),
		cmocka_unit_test(test_strtod),
		cmocka_unit_test(test_dtostr),
		cmocka_unit_test(test_formatted_filename_before_startup),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
