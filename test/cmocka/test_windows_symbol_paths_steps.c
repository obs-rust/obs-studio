#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <cmocka.h>

#include <string.h>

#include "obs-windows-symbol-paths.h"

/*
 * Characterization of the debug symbol search path rules of
 * libobs/obs-windows.c's reset_win32_symbol_paths(), moved to
 * libobs/obs-windows-symbol-paths.c before their Rust port:
 * - obs_windows_symbol_paths_dir(): the directory of a module's binary;
 * - obs_windows_symbol_paths_listed(): whether a directory is listed already;
 * - obs_windows_symbol_paths_join(): the search path.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions take no Windows type and are hidden in libobs, so this test
 * compiles the C source into itself, calls only them, and runs on every OS.
 * dstr comes from libobs.
 *
 * Not characterized: a NULL or empty bin_path. dstr_copy() leaves the array
 * NULL for it and the C then calls strrchr() on NULL. obs-module.c sets
 * bin_path only after the module's library opened, so no module reaches it.
 */

/* Whether bin_path gives a directory, with the C's copy in a fresh dir. */
static bool dir_of(const char *bin_path, struct dstr *dir)
{
	*dir = (struct dstr){0};
	return obs_windows_symbol_paths_dir(bin_path, dir);
}

static void dir_cut(void **state)
{
	struct dstr dir;
	(void)state;

	assert_true(dir_of("C:\\obs\\obs-plugins\\64bit\\a.dll", &dir));
	assert_string_equal(dir.array, "C:\\obs\\obs-plugins\\64bit");
	/* characterized, not endorsed: the NUL is written in place and len
	 * keeps the whole path's length (the caller reads only the array) */
	assert_int_equal(dir.len, strlen("C:\\obs\\obs-plugins\\64bit\\a.dll"));
	dstr_free(&dir);

	assert_true(dir_of("obs-plugins\\a.dll", &dir));
	assert_string_equal(dir.array, "obs-plugins");
	dstr_free(&dir);
}

static void dir_slashes(void **state)
{
	struct dstr dir;
	(void)state;

	assert_true(dir_of("C:/obs/obs-plugins/64bit/a.dll", &dir));
	assert_string_equal(dir.array, "C:\\obs\\obs-plugins\\64bit");
	assert_int_equal(dir.len, strlen("C:/obs/obs-plugins/64bit/a.dll"));
	dstr_free(&dir);

	assert_true(dir_of("C:/obs\\obs-plugins/64bit\\a.dll", &dir));
	assert_string_equal(dir.array, "C:\\obs\\obs-plugins\\64bit");
	dstr_free(&dir);

	assert_true(dir_of("../../obs-plugins/64bit/a.dll", &dir));
	assert_string_equal(dir.array, "..\\..\\obs-plugins\\64bit");
	dstr_free(&dir);
}

static void dir_none(void **state)
{
	struct dstr dir;
	(void)state;

	/* no separator: no directory, and dir keeps the copy */
	assert_false(dir_of("a.dll", &dir));
	assert_string_equal(dir.array, "a.dll");
	assert_int_equal(dir.len, 5);
	dstr_free(&dir);

	assert_false(dir_of("C:a.dll", &dir));
	assert_string_equal(dir.array, "C:a.dll");
	dstr_free(&dir);

	assert_false(dir_of(" ", &dir));
	assert_string_equal(dir.array, " ");
	dstr_free(&dir);
}

static void dir_trailing(void **state)
{
	struct dstr dir;
	(void)state;

	/* characterized, not endorsed: a trailing separator cuts to the path
	 * itself, without its last separator */
	assert_true(dir_of("C:\\obs\\", &dir));
	assert_string_equal(dir.array, "C:\\obs");
	dstr_free(&dir);

	assert_true(dir_of("C:/obs//", &dir));
	assert_string_equal(dir.array, "C:\\obs\\");
	dstr_free(&dir);

	assert_true(dir_of("\\", &dir));
	assert_string_equal(dir.array, "");
	assert_int_equal(dir.len, 1);
	dstr_free(&dir);
}

static void dir_root(void **state)
{
	struct dstr dir;
	(void)state;

	/* characterized, not endorsed: a root file gives the drive without
	 * its separator, which os_get_abs_path_ptr() turns into that drive's
	 * current directory */
	assert_true(dir_of("C:\\a.dll", &dir));
	assert_string_equal(dir.array, "C:");
	dstr_free(&dir);

	assert_true(dir_of("/a.dll", &dir));
	assert_string_equal(dir.array, "");
	dstr_free(&dir);
}

static void dir_unc(void **state)
{
	struct dstr dir;
	(void)state;

	assert_true(dir_of("\\\\server\\share\\obs\\a.dll", &dir));
	assert_string_equal(dir.array, "\\\\server\\share\\obs");
	dstr_free(&dir);

	assert_true(dir_of("//server/share/a.dll", &dir));
	assert_string_equal(dir.array, "\\\\server\\share");
	dstr_free(&dir);
}

static void dir_bytes(void **state)
{
	struct dstr dir;
	(void)state;

	/* non-ASCII bytes are copied as they are; only '/' changes */
	assert_true(dir_of("C:\\\xc3\x9c"
			   "bung/\xe6\x8f\x92\xe4\xbb\xb6\\a.dll",
			   &dir));
	assert_string_equal(dir.array, "C:\\\xc3\x9c"
				       "bung\\\xe6\x8f\x92\xe4\xbb\xb6");
	dstr_free(&dir);
}

static void listed_matches(void **state)
{
	char *paths[] = {"C:\\cwd", "C:\\obs\\obs-plugins\\64bit", "D:\\plugins"};
	(void)state;

	assert_true(obs_windows_symbol_paths_listed(paths, 3, "C:\\cwd"));
	assert_true(obs_windows_symbol_paths_listed(paths, 3, "D:\\plugins"));
	assert_true(obs_windows_symbol_paths_listed(paths, 3, "C:\\obs\\obs-plugins\\64bit"));

	/* ASCII case does not count */
	assert_true(obs_windows_symbol_paths_listed(paths, 3, "c:\\OBS\\Obs-Plugins\\64BIT"));
	assert_true(obs_windows_symbol_paths_listed(paths, 3, "d:\\PLUGINS"));

	/* a prefix, an extension or a separator is not a match */
	assert_false(obs_windows_symbol_paths_listed(paths, 3, "C:\\cw"));
	assert_false(obs_windows_symbol_paths_listed(paths, 3, "C:\\cwd2"));
	assert_false(obs_windows_symbol_paths_listed(paths, 3, "C:\\cwd\\"));
	assert_false(obs_windows_symbol_paths_listed(paths, 3, "C:/cwd"));
	assert_false(obs_windows_symbol_paths_listed(paths, 3, ""));
}

static void listed_bounds(void **state)
{
	char *paths[] = {"C:\\cwd", "D:\\plugins"};
	(void)state;

	/* only the first num entries */
	assert_false(obs_windows_symbol_paths_listed(paths, 1, "D:\\plugins"));
	assert_true(obs_windows_symbol_paths_listed(paths, 2, "D:\\plugins"));

	/* an empty list */
	assert_false(obs_windows_symbol_paths_listed(NULL, 0, "C:\\cwd"));
	assert_false(obs_windows_symbol_paths_listed(paths, 0, "C:\\cwd"));
}

static void listed_case(void **state)
{
	char *paths[] = {"C:\\\xc3\x9c"
			 "bung",
			 "C:\\\xc3\xa9t\xc3\xa9"};
	(void)state;

	assert_true(obs_windows_symbol_paths_listed(paths, 2,
						    "c:\\\xc3\x9c"
						    "BUNG"));

	/* characterized, not endorsed: astrcmpi() folds ASCII only, so a
	 * directory that differs in a non-ASCII letter's case is listed again
	 * (a duplicate search path entry is harmless) */
	assert_false(obs_windows_symbol_paths_listed(paths, 2,
						     "C:\\\xc3\xbc"
						     "bung"));
	assert_false(obs_windows_symbol_paths_listed(paths, 2, "C:\\\xc3\x89t\xc3\x89"));
}

static void listed_empty_entries(void **state)
{
	char *paths[] = {NULL, ""};
	(void)state;

	/* astrcmpi() reads NULL as "" */
	assert_true(obs_windows_symbol_paths_listed(paths, 1, ""));
	assert_true(obs_windows_symbol_paths_listed(paths + 1, 1, ""));
	assert_false(obs_windows_symbol_paths_listed(paths, 2, "C:\\cwd"));
}

/* The search path joined from num paths into an empty dstr. */
static void join_of(char *const *paths, size_t num, struct dstr *out)
{
	*out = (struct dstr){0};
	obs_windows_symbol_paths_join(paths, num, out);
}

static void join_counts(void **state)
{
	char *paths[] = {"C:\\cwd", "C:\\obs\\obs-plugins\\64bit", "D:\\plugins"};
	struct dstr out;
	(void)state;

	/* nothing to join: the array stays NULL, and no search path is set */
	join_of(NULL, 0, &out);
	assert_null(out.array);
	assert_int_equal(out.len, 0);

	join_of(paths, 1, &out);
	assert_string_equal(out.array, "C:\\cwd");
	assert_int_equal(out.len, 6);
	dstr_free(&out);

	join_of(paths, 3, &out);
	assert_string_equal(out.array, "C:\\cwd;C:\\obs\\obs-plugins\\64bit;D:\\plugins");
	assert_int_equal(out.len, strlen(out.array));
	dstr_free(&out);
}

static void join_skips(void **state)
{
	char *middle_empty[] = {"a", "", "c"};
	char *middle_null[] = {"a", NULL, "c"};
	char *all_empty[] = {"", NULL, ""};
	struct dstr out;
	(void)state;

	join_of(middle_empty, 3, &out);
	assert_string_equal(out.array, "a;c");
	dstr_free(&out);

	join_of(middle_null, 3, &out);
	assert_string_equal(out.array, "a;c");
	dstr_free(&out);

	join_of(all_empty, 3, &out);
	assert_null(out.array);
}

static void join_first_entry(void **state)
{
	char *first_empty[] = {"", "b", "c"};
	char *first_null[] = {NULL, "b"};
	struct dstr out;
	(void)state;

	/* characterized, not endorsed: ';' goes before every entry but index
	 * 0, so a skipped first entry leaves a leading ';' (index 0 is the
	 * working directory, which is never empty in practice) */
	join_of(first_empty, 3, &out);
	assert_string_equal(out.array, ";b;c");
	dstr_free(&out);

	join_of(first_null, 2, &out);
	assert_string_equal(out.array, ";b");
	dstr_free(&out);
}

static void join_appends(void **state)
{
	char *paths[] = {"a", "b"};
	struct dstr out = {0};
	(void)state;

	/* the paths go after what out holds, with no separator before them */
	dstr_copy(&out, "x");
	obs_windows_symbol_paths_join(paths, 2, &out);
	assert_string_equal(out.array, "xa;b");
	assert_int_equal(out.len, 4);
	dstr_free(&out);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(dir_cut),
		cmocka_unit_test(dir_slashes),
		cmocka_unit_test(dir_none),
		cmocka_unit_test(dir_trailing),
		cmocka_unit_test(dir_root),
		cmocka_unit_test(dir_unc),
		cmocka_unit_test(dir_bytes),
		cmocka_unit_test(listed_matches),
		cmocka_unit_test(listed_bounds),
		cmocka_unit_test(listed_case),
		cmocka_unit_test(listed_empty_entries),
		cmocka_unit_test(join_counts),
		cmocka_unit_test(join_skips),
		cmocka_unit_test(join_first_entry),
		cmocka_unit_test(join_appends),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
