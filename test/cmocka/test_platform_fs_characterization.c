#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include <obs-avc.h>
#include <util/array-serializer.h>
#include <util/bmem.h>
#include <util/dstr.h>
#include <util/platform.h>
#include <util/serializer.h>

/*
 * Characterization tests for the filesystem helpers in util/platform-nix.c.h
 * (and their platform-windows.c.h twins) that no other test covers, plus
 * array_output_serializer_reset, obs_avc_find_startcode and dstr_vcatf. They
 * pin the current C behavior for the Rust port; cases marked "characterized,
 * not endorsed" are odd but real.
 *
 * Everything lives under a temp dir relative to cwd, with '/' separators.
 */

#define ROOT "test_platform_fs_characterization.tmp"
#define P_A ROOT "/a.txt"
#define P_B ROOT "/b.txt"
#define P_COPY ROOT "/copy.txt"
#define P_SUB ROOT "/sub.txt"
#define P_MISSING ROOT "/missing.txt"

static void write_file(const char *path, const char *text)
{
	FILE *f = os_fopen(path, "wb");
	if (!f) {
		fprintf(stderr, "cannot create %s\n", path);
		fail();
	}
	fwrite(text, 1, strlen(text), f);
	fclose(f);
}

static void cleanup(void)
{
	os_unlink(P_A);
	os_unlink(P_B);
	os_unlink(P_COPY);
	os_rmdir(P_SUB);
	os_rmdir(ROOT);
}

static int setup(void **state)
{
	(void)state;
	cleanup();
	if (os_mkdir(ROOT) == MKDIR_ERROR)
		return -1;
	write_file(P_A, "alpha");
	write_file(P_B, "bravo-bravo");
	if (os_mkdir(P_SUB) == MKDIR_ERROR)
		return -1;
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	cleanup();
	return 0;
}

static void test_is_obs_plugin(void **state)
{
	(void)state;
#ifndef _WIN32
	/* POSIX ignores the path entirely. characterized, not endorsed */
	assert_true(os_is_obs_plugin(P_A));
	assert_true(os_is_obs_plugin(P_MISSING));
	assert_true(os_is_obs_plugin(NULL));
#endif
}

static void test_getcwd_chdir(void **state)
{
	(void)state;
	char orig[1024];
	char inside[1024];

	assert_ptr_equal(os_getcwd(orig, sizeof(orig)), orig);
	assert_int_equal(os_chdir(ROOT), 0);
	assert_ptr_equal(os_getcwd(inside, sizeof(inside)), inside);
	assert_true(strlen(inside) > strlen(ROOT));
	assert_string_equal(inside + strlen(inside) - strlen(ROOT), ROOT);
	assert_int_equal(os_chdir(orig), 0);
	assert_true(os_file_exists(P_A));
#ifndef _WIN32
	assert_int_not_equal(os_chdir(P_MISSING), 0);
	char tiny[2];
	assert_null(os_getcwd(tiny, sizeof(tiny)));
#endif
}

static void test_get_abs_path(void **state)
{
	(void)state;
	char cwd[1024];
	char expected[1200];
	char out[1200];

	assert_non_null(os_getcwd(cwd, sizeof(cwd)));
	assert_int_equal(os_get_abs_path(P_A, NULL, 16), 0);
#ifndef _WIN32
	snprintf(expected, sizeof(expected), "%s/%s", cwd, P_A);
	size_t n = os_get_abs_path(P_A, out, sizeof(out));
	assert_int_equal(n, strlen(expected));
	assert_string_equal(out, expected);

	/* realpath() needs the file to exist */
	assert_int_equal(os_get_abs_path(P_MISSING, out, sizeof(out)), 0);
	assert_null(os_get_abs_path_ptr(P_MISSING));

	/* Truncates, but returns the untruncated length. characterized, not endorsed */
	char small[4];
	assert_int_equal(os_get_abs_path(P_A, small, sizeof(small)), strlen(expected));
	assert_int_equal(strlen(small), 3);
	assert_memory_equal(small, expected, 3);

	char *p = os_get_abs_path_ptr(P_A);
	assert_non_null(p);
	assert_string_equal(p, expected);
	bfree(p);
#else
	(void)expected;
	assert_true(os_get_abs_path(P_A, out, sizeof(out)) > 0);
	char *p = os_get_abs_path_ptr(P_A);
	assert_non_null(p);
	bfree(p);
#endif
}

static void test_glob(void **state)
{
	(void)state;
	os_glob_t *g = NULL;

	assert_int_equal(os_glob(ROOT "/*.txt", 0, &g), 0);
	assert_non_null(g);
	assert_int_equal(g->gl_pathc, 3); /* a.txt, b.txt, sub.txt (a directory) */
	size_t dirs = 0;
	for (size_t i = 0; i < g->gl_pathc; i++) {
		if (g->gl_pathv[i].directory)
			dirs++;
	}
	assert_int_equal(dirs, 1);
#ifndef _WIN32
	/* glob() sorts and returns the pattern's own prefix */
	assert_string_equal(g->gl_pathv[0].path, P_A);
	assert_false(g->gl_pathv[0].directory);
	assert_string_equal(g->gl_pathv[1].path, P_B);
	assert_string_equal(g->gl_pathv[2].path, P_SUB);
	assert_true(g->gl_pathv[2].directory);
#endif
	os_globfree(g);
	os_globfree(NULL);

#ifndef _WIN32
	g = (os_glob_t *)(uintptr_t)1;
	assert_int_not_equal(os_glob(ROOT "/*.nomatch", 0, &g), 0);
	assert_null(g);
#endif
}

static void test_copyfile(void **state)
{
	(void)state;
	assert_int_equal(os_copyfile(P_B, P_COPY), 0);
	char *text = os_quick_read_utf8_file(P_COPY);
	assert_non_null(text);
	assert_string_equal(text, "bravo-bravo");
	bfree(text);

	/* An existing destination is never overwritten */
	assert_int_not_equal(os_copyfile(P_A, P_COPY), 0);
	text = os_quick_read_utf8_file(P_COPY);
	assert_non_null(text);
	assert_string_equal(text, "bravo-bravo");
	bfree(text);

	assert_int_not_equal(os_copyfile(P_MISSING, ROOT "/never.txt"), 0);
	assert_false(os_file_exists(ROOT "/never.txt"));
}

static void test_sleepto_ns_fast(void **state)
{
	(void)state;
	/* Only that it returns; a target in the past returns false on POSIX. */
	uint64_t now = os_gettime_ns();
	(void)os_sleepto_ns_fast(now + 1000000);
#ifndef _WIN32
	assert_false(os_sleepto_ns_fast(0));
#endif
}

static void test_array_serializer_reset(void **state)
{
	(void)state;
	struct serializer s;
	struct array_output_data data;
	array_output_serializer_init(&s, &data);

	s_w8(&s, 1);
	s_w8(&s, 2);
	s_w8(&s, 3);
	assert_int_equal(data.bytes.num, 3);
	assert_int_equal(data.cur_pos, 3);

	array_output_serializer_reset(&data);
	assert_int_equal(data.bytes.num, 0);
	assert_int_equal(data.cur_pos, 0);

	s_w8(&s, 9);
	assert_int_equal(data.bytes.num, 1);
	assert_int_equal(data.bytes.array[0], 9);
	assert_int_equal(data.cur_pos, 1);

	array_output_serializer_free(&data);
}

static void test_avc_find_startcode(void **state)
{
	(void)state;
	static const uint8_t three[] = {0x11, 0x22, 0x33, 0x44, 0, 0, 1, 0x65, 0x77, 0x88, 0x99, 0xaa};
	static const uint8_t four[] = {0x11, 0x22, 0x33, 0, 0, 0, 1, 0x65, 0x77, 0x88, 0x99, 0xaa};
	static const uint8_t none[] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12};
	static const uint8_t first[] = {0, 0, 0, 1, 0x67, 0x42, 0, 0, 1, 0x68, 0xce, 0x3c};

	assert_ptr_equal(obs_avc_find_startcode(three, three + sizeof(three)), three + 4);
	/* The zero before 00 00 01 is included (4-byte start code) */
	assert_ptr_equal(obs_avc_find_startcode(four, four + sizeof(four)), four + 3);
	assert_ptr_equal(obs_avc_find_startcode(none, none + sizeof(none)), none + sizeof(none));
	assert_ptr_equal(obs_avc_find_startcode(first, first + sizeof(first)), first);
	assert_ptr_equal(obs_avc_find_startcode(first + 4, first + sizeof(first)), first + 6);
	/* Empty and inverted ranges return end */
	assert_ptr_equal(obs_avc_find_startcode(three, three), three);
	assert_ptr_equal(obs_avc_find_startcode(three + 5, three + 2), three + 2);
	assert_ptr_equal(obs_avc_find_startcode(NULL, NULL), NULL);
}

static void catf_wrapper(struct dstr *dst, const char *format, ...)
{
	va_list args;
	va_start(args, format);
	dstr_vcatf(dst, format, args);
	va_end(args);
}

static void test_dstr_vcatf(void **state)
{
	(void)state;
	struct dstr s = {0};

	catf_wrapper(&s, "%d-%s", 42, "x");
	assert_string_equal(s.array, "42-x");
	assert_int_equal(s.len, 4);

	catf_wrapper(&s, "|%03u|%c", 7u, 'z');
	assert_string_equal(s.array, "42-x|007|z");
	assert_int_equal(s.len, 10);

	catf_wrapper(&s, "%s", "");
	assert_string_equal(s.array, "42-x|007|z");
	assert_int_equal(s.len, 10);
	dstr_free(&s);

	/* Appending nothing to an empty string frees it. characterized, not endorsed */
	struct dstr e = {0};
	catf_wrapper(&e, "%s", "");
	assert_null(e.array);
	assert_int_equal(e.len, 0);
	dstr_free(&e);
}

int main(void)
{
	const struct CMUnitTest fs_tests[] = {
		cmocka_unit_test(test_is_obs_plugin), cmocka_unit_test(test_getcwd_chdir),
		cmocka_unit_test(test_get_abs_path),  cmocka_unit_test(test_glob),
		cmocka_unit_test(test_copyfile),      cmocka_unit_test(test_sleepto_ns_fast),
	};
	const struct CMUnitTest misc_tests[] = {
		cmocka_unit_test(test_array_serializer_reset),
		cmocka_unit_test(test_avc_find_startcode),
		cmocka_unit_test(test_dstr_vcatf),
	};

	int failed = cmocka_run_group_tests_name("platform_fs", fs_tests, setup, teardown);
	failed += cmocka_run_group_tests_name("leftovers", misc_tests, NULL, NULL);
	return failed;
}
