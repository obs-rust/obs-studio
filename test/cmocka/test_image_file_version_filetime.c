/* Follow-up to the fix for obsproject/obs-studio#10700 (reported by
 * chocobclyn, on Windows 11): an Image source whose file is replaced within
 * the same second keeps showing the old image.
 *
 * The fix compares the sub-second part of the file's modification time, but
 * on Windows libobs's os_stat() reports whole seconds only, so there an
 * image replaced by one of the same size in the same second was still
 * missed. On Windows the source now reads the file's last write time
 * (100 ns resolution); image_file_version_from_filetime() turns it into the
 * version it compares. That conversion is plain arithmetic, so it is tested
 * here on every platform:
 *
 *   - two write times 0.4 s apart in the same second: different versions;
 *   - the sub-second part is kept to the 100 ns tick;
 *   - control: whole seconds convert to the Unix time stat() reports.
 *
 * The last case reads a real file with image_file_version_get(), the
 * Windows code on Windows and stat() elsewhere. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>

#include "image-file-version.h"

/* 2023-11-14 22:13:20 UTC: Unix time 1700000000. */
#define UNIX_SEC 1700000000LL
#define FILETIME_OF(sec) (((uint64_t)(sec) + 11644473600ULL) * 10000000ULL)

/* Replaced 0.4 s later in the same second, same size. */
static void same_second_write_times_differ(void **state)
{
	(void)state;
	struct image_file_version before, after;

	image_file_version_from_filetime(&before, FILETIME_OF(UNIX_SEC) + 1000000, 4096);
	image_file_version_from_filetime(&after, FILETIME_OF(UNIX_SEC) + 5000000, 4096);

	assert_int_equal(before.mtime_sec, after.mtime_sec);
	assert_false(image_file_version_equal(&before, &after));
}

static void sub_second_part_is_kept(void **state)
{
	(void)state;
	struct image_file_version v;

	image_file_version_from_filetime(&v, FILETIME_OF(UNIX_SEC) + 1234567, 1);
	assert_int_equal(v.mtime_sec, UNIX_SEC);
	assert_int_equal(v.mtime_nsec, 123456700);
}

/* Control: whole seconds, size and existence. */
static void whole_seconds_convert_to_unix_time(void **state)
{
	(void)state;
	struct image_file_version v;

	image_file_version_from_filetime(&v, FILETIME_OF(UNIX_SEC), 4096);
	assert_true(v.exists);
	assert_int_equal(v.mtime_sec, UNIX_SEC);
	assert_int_equal(v.mtime_nsec, 0);
	assert_int_equal(v.size, 4096);

	image_file_version_from_filetime(&v, FILETIME_OF(0), 0);
	assert_int_equal(v.mtime_sec, 0);
}

/* Control: a real file through the platform's code. */
static void real_file_is_read(void **state)
{
	(void)state;
	const char *path = "test_image_file_version_filetime.tmp";
	struct image_file_version v, missing;

	FILE *f = fopen(path, "wb");
	assert_non_null(f);
	fwrite("0123456789", 1, 10, f);
	fclose(f);

	image_file_version_get(path, &v);
	remove(path);
	image_file_version_get(path, &missing);

	assert_true(v.exists);
	assert_int_equal(v.size, 10);
	assert_true(v.mtime_sec > UNIX_SEC);
	assert_false(missing.exists);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(same_second_write_times_differ),
		cmocka_unit_test(sub_second_part_is_kept),
		cmocka_unit_test(whole_seconds_convert_to_unix_time),
		cmocka_unit_test(real_file_is_read),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
