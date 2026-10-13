/* Regression test for obsproject/obs-studio#10700 (reported by
 * chocobclyn): an Image source whose file is deleted and replaced by a new
 * file of the same name keeps showing the old image.
 *
 * The source checks its file once a second and loads it again when the
 * file's modification time changed. That time was compared in whole
 * seconds, so a new file written in the same second as the one the source
 * had loaded looked unchanged, and the old image stayed until the file
 * changed again. Tools that update an overlay image by deleting it and
 * writing a new one, or by overwriting it, often do that within a second.
 *
 * The test sets the modification times of real files explicitly and checks
 * what the source compares (image_file_version_get/equal, the helper
 * image-source.c calls):
 *
 *   - a new file 0.4 s after the old one, same second, same size: changed;
 *   - a new file with the same modification time but another size (file
 *     systems that keep whole or even two seconds): changed;
 *   - controls: an untouched file is unchanged; a deleted file is changed,
 *     and so is a new file after it; a file that never existed is equal to
 *     itself. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

#include "image-file-version.h"

#define BASE_SEC 1700000000

static char dir[256];
static char path[300];

static void write_file(size_t size, char fill)
{
	FILE *f = fopen(path, "wb");
	assert_non_null(f);
	for (size_t i = 0; i < size; i++)
		fputc(fill, f);
	fclose(f);
}

static void set_mtime(long sec, long nsec)
{
	struct timespec times[2] = {{.tv_sec = sec, .tv_nsec = nsec}, {.tv_sec = sec, .tv_nsec = nsec}};
	assert_int_equal(utimensat(AT_FDCWD, path, times, 0), 0);
}

static bool fs_keeps_nsec(void)
{
	write_file(16, 'x');
	set_mtime(BASE_SEC, 123456789);
	struct stat st;
	assert_int_equal(stat(path, &st), 0);
#ifdef __APPLE__
	bool keeps = st.st_mtimespec.tv_nsec != 0;
#else
	bool keeps = st.st_mtim.tv_nsec != 0;
#endif
	unlink(path);
	return keeps;
}

/* Deleted and replaced by a new file 0.4 s later, in the same second, of
 * the same size (an image of the same dimensions and format). */
static void replaced_in_same_second_is_changed(void **state)
{
	(void)state;
	struct image_file_version before, after;

	if (!fs_keeps_nsec())
		skip();

	write_file(64, 'a');
	set_mtime(BASE_SEC, 100000000);
	image_file_version_get(path, &before);

	unlink(path);
	write_file(64, 'b');
	set_mtime(BASE_SEC, 500000000);
	image_file_version_get(path, &after);

	assert_true(before.exists);
	assert_true(after.exists);
	assert_false(image_file_version_equal(&before, &after));
}

/* File systems that keep whole (or two) seconds give both files the same
 * time; a different size still shows the change. */
static void same_time_other_size_is_changed(void **state)
{
	(void)state;
	struct image_file_version before, after;

	write_file(64, 'a');
	set_mtime(BASE_SEC, 0);
	image_file_version_get(path, &before);

	unlink(path);
	write_file(80, 'b');
	set_mtime(BASE_SEC, 0);
	image_file_version_get(path, &after);

	assert_false(image_file_version_equal(&before, &after));
}

/* Control: nothing happened to the file. */
static void untouched_file_is_unchanged(void **state)
{
	(void)state;
	struct image_file_version before, after;

	write_file(64, 'a');
	set_mtime(BASE_SEC, 100000000);
	image_file_version_get(path, &before);
	image_file_version_get(path, &after);

	assert_true(image_file_version_equal(&before, &after));
}

/* Control: deleting the file is a change, and so is a new file after it,
 * even one with the old time and size. */
static void deleted_then_recreated_is_changed(void **state)
{
	(void)state;
	struct image_file_version present, missing, back;

	write_file(64, 'a');
	set_mtime(BASE_SEC, 100000000);
	image_file_version_get(path, &present);

	unlink(path);
	image_file_version_get(path, &missing);
	assert_false(missing.exists);
	assert_false(image_file_version_equal(&present, &missing));

	write_file(64, 'b');
	set_mtime(BASE_SEC, 100000000);
	image_file_version_get(path, &back);
	assert_false(image_file_version_equal(&missing, &back));
}

/* Control: a file that is not there stays "not there". */
static void missing_file_is_stable(void **state)
{
	(void)state;
	struct image_file_version a, b;

	unlink(path);
	image_file_version_get(path, &a);
	image_file_version_get(path, &b);
	assert_true(image_file_version_equal(&a, &b));
	image_file_version_get(NULL, &a);
	assert_false(a.exists);
}

static int setup(void **state)
{
	(void)state;
	const char *tmp = getenv("TMPDIR");
	snprintf(dir, sizeof(dir), "%s/image-file-version-XXXXXX", tmp && *tmp ? tmp : "/tmp");
	if (!mkdtemp(dir))
		return -1;
	snprintf(path, sizeof(path), "%s/image.png", dir);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	unlink(path);
	rmdir(dir);
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(replaced_in_same_second_is_changed),
		cmocka_unit_test(same_time_other_size_is_changed),
		cmocka_unit_test(untouched_file_is_unchanged),
		cmocka_unit_test(deleted_then_recreated_is_changed),
		cmocka_unit_test(missing_file_is_stable),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
