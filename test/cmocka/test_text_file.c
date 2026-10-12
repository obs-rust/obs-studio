#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <wchar.h>
#include <cmocka.h>

#include <util/bmem.h>
#include <util/platform.h>

#include "text-file.h"

/* The text files of the FreeType 2 text source: UTF-8, or UTF-16LE with a
 * BOM, carriage returns removed, optionally only the last lines.
 *
 * Regressions: UTF-16 files were copied into the wchar_t buffer as raw
 * 2-byte units, which is wrong where wchar_t is 4 bytes and left the text
 * without a terminator; an odd-sized UTF-16 file also over-read on Windows
 * and made the backwards scan of ft2_read_text_file_end wrap around.
 *
 * NUL bytes (obsproject/obs-studio#7595, reported by raleighlittles): a file
 * with NUL padding, such as a device's descriptor dump, showed only the text
 * before the first NUL, because the UTF-8 reader took the length with
 * strlen(). NULs cannot be shown, so they are dropped and the rest of the
 * file is read.
 *
 * A file that does not exist yet (obsproject/obs-studio#3196, reported by
 * JPLeBreton): with "Read from file" set to a path that another program
 * writes later, such as a now-playing or level-info file, the text never
 * appeared until OBS restarted, because the path was only kept when the
 * file existed and the source polled nothing. The source now keeps polling
 * the path and reads the file once it appears. */

#define TEXT_PATH "test_text_file.txt"

static void write_bytes(const void *data, size_t size)
{
	FILE *file = fopen(TEXT_PATH, "wb");
	assert_non_null(file);
	assert_int_equal(fwrite(data, 1, size, file), size);
	fclose(file);
}

static void write_utf8(const char *text)
{
	write_bytes(text, strlen(text));
}

/* BOM, then each UTF-16 unit little-endian, then `extra` stray bytes */
static void write_utf16(const uint16_t *units, size_t count, size_t extra)
{
	uint8_t data[256];
	size_t size = 0;
	data[size++] = 0xff;
	data[size++] = 0xfe;
	for (size_t i = 0; i < count; i++) {
		data[size++] = (uint8_t)units[i];
		data[size++] = (uint8_t)(units[i] >> 8);
	}
	for (size_t i = 0; i < extra; i++)
		data[size++] = 'x';
	assert_true(size <= sizeof(data));
	write_bytes(data, size);
}

/* an ASCII string as UTF-16 units */
static size_t ascii_units(const char *text, uint16_t *units)
{
	size_t n = 0;
	while (text[n]) {
		units[n] = (uint8_t)text[n];
		n++;
	}
	return n;
}

static void write_utf16_ascii(const char *text, size_t extra)
{
	uint16_t units[100];
	write_utf16(units, ascii_units(text, units), extra);
}

/* `got` is the wide string of the UTF-8 `want`; frees `got` */
static void expect_text(wchar_t *got, const char *want)
{
	wchar_t *want_wcs;
	os_utf8_to_wcs_ptr(want, 0, &want_wcs);
	assert_non_null(got);
	assert_int_equal(wcslen(got), wcslen(want_wcs));
	assert_memory_equal(got, want_wcs, (wcslen(want_wcs) + 1) * sizeof(wchar_t));
	bfree(want_wcs);
	bfree(got);
}

static int teardown(void **state)
{
	(void)state;
	remove(TEXT_PATH);
	return 0;
}

static void test_utf8_file(void **state)
{
	(void)state;

	write_utf8("one\r\ntwo \xc3\xa9\n");
	expect_text(ft2_read_text_file(TEXT_PATH), "one\ntwo \xc3\xa9\n");

	write_utf8("");
	expect_text(ft2_read_text_file(TEXT_PATH), "");

	remove(TEXT_PATH);
	assert_null(ft2_read_text_file(TEXT_PATH));
	assert_null(ft2_read_text_file_end(TEXT_PATH, 1));
}

static void test_utf16_file(void **state)
{
	(void)state;

	write_utf16_ascii("one\r\ntwo", 0);
	expect_text(ft2_read_text_file(TEXT_PATH), "one\ntwo");

	/* U+00E9 and U+1F600, a surrogate pair */
	const uint16_t units[] = {'a', 0xe9, 0xd83d, 0xde00, 'b'};
	write_utf16(units, 5, 0);
	expect_text(ft2_read_text_file(TEXT_PATH), "a\xc3\xa9\xf0\x9f\x98\x80"
						   "b");

	write_utf16(NULL, 0, 0);
	expect_text(ft2_read_text_file(TEXT_PATH), "");
}

/* A stray last byte is not half of a character. */
static void test_utf16_odd_size(void **state)
{
	(void)state;

	write_utf16_ascii("h", 1);
	expect_text(ft2_read_text_file(TEXT_PATH), "h");
	expect_text(ft2_read_text_file_end(TEXT_PATH, 10), "h");

	write_utf16(NULL, 0, 1);
	expect_text(ft2_read_text_file(TEXT_PATH), "");
	expect_text(ft2_read_text_file_end(TEXT_PATH, 10), "");
}

/* log_lines counts line breaks from the end, so the text keeps one more
 * line than log_lines when the file does not end with one. */
static void test_utf8_end(void **state)
{
	(void)state;

	write_utf8("a\nb\nc");
	expect_text(ft2_read_text_file_end(TEXT_PATH, 0), "c");
	expect_text(ft2_read_text_file_end(TEXT_PATH, 1), "b\nc");
	expect_text(ft2_read_text_file_end(TEXT_PATH, 5), "a\nb\nc");

	write_utf8("a\r\nb\r\nc\r\n");
	expect_text(ft2_read_text_file_end(TEXT_PATH, 1), "c\n");
}

/* The same lines, as UTF-16, give the same text (without the BOM). */
static void test_utf16_end_matches_utf8(void **state)
{
	(void)state;

	const char *texts[] = {"a\nb\nc", "a\r\nbb\nccc\n\ndd\n", "\nx", "single", ""};
	for (size_t t = 0; t < sizeof(texts) / sizeof(texts[0]); t++) {
		for (uint32_t lines = 0; lines < 7; lines++) {
			write_utf8(texts[t]);
			wchar_t *want = ft2_read_text_file_end(TEXT_PATH, lines);
			char *want_utf8;
			os_wcs_to_utf8_ptr(want, 0, &want_utf8);
			bfree(want);

			write_utf16_ascii(texts[t], 0);
			expect_text(ft2_read_text_file_end(TEXT_PATH, lines), want_utf8);
			bfree(want_utf8);
		}
	}
}

/* NUL-padded lines, as in the report: every line is read. */
static void test_utf8_nul_bytes(void **state)
{
	(void)state;

	const char padded[] = "first\0\0\0\nsecond\0\0\nthird\n";
	write_bytes(padded, sizeof(padded) - 1);
	expect_text(ft2_read_text_file(TEXT_PATH), "first\nsecond\nthird\n");
	expect_text(ft2_read_text_file_end(TEXT_PATH, 1), "third\n");
	expect_text(ft2_read_text_file_end(TEXT_PATH, 2), "second\nthird\n");

	const char leading[] = "\0\0text \xc3\xa9\0";
	write_bytes(leading, sizeof(leading) - 1);
	expect_text(ft2_read_text_file(TEXT_PATH), "text \xc3\xa9");
	expect_text(ft2_read_text_file_end(TEXT_PATH, 3), "text \xc3\xa9");
}

static void test_utf16_nul_units(void **state)
{
	(void)state;

	const uint16_t units[] = {'a', 0, 0, '\n', 'b', 0, '\n', 'c'};
	write_utf16(units, 8, 0);
	expect_text(ft2_read_text_file(TEXT_PATH), "a\nb\nc");
	expect_text(ft2_read_text_file_end(TEXT_PATH, 1), "b\nc");
}

#define SECOND 1000000000ULL

/* Tick the watch once per simulated second, for up to `seconds`; true as
 * soon as it asks for the file to be reread. */
static bool reload_within(struct ft2_text_file_watch *watch, uint64_t *now, int seconds)
{
	for (int i = 0; i < seconds; i++) {
		*now += SECOND;
		if (ft2_text_file_watch_tick(watch, *now))
			return true;
	}
	return false;
}

/* The source rereads the file it watches, as its tick does. */
static void expect_reload(struct ft2_text_file_watch *watch, uint64_t *now, const char *want)
{
	if (!reload_within(watch, now, 3)) {
		fprintf(stderr, "the text file %s was not reread within 3 s of %s\n", TEXT_PATH,
			watch->path ? "changing" : "appearing (the source polls no path)");
		fail();
	}
	assert_non_null(watch->path);
	assert_string_equal(watch->path, TEXT_PATH);
	expect_text(ft2_read_text_file(watch->path), want);
}

/* Control: the file exists when the source is set up, then changes. */
static void test_watch_existing_file(void **state)
{
	(void)state;
	struct ft2_text_file_watch watch = {0};
	uint64_t now = 100 * SECOND;

	write_utf8("now playing: one");
	assert_int_equal(ft2_text_file_watch_set(&watch, TEXT_PATH, false, now), FT2_TEXT_FILE_LOAD);
	assert_int_equal(ft2_text_file_watch_set(&watch, TEXT_PATH, false, now), FT2_TEXT_FILE_UNCHANGED);
	assert_int_equal(ft2_text_file_watch_set(&watch, TEXT_PATH, true, now), FT2_TEXT_FILE_LOAD);
	expect_reload(&watch, &now, "now playing: one");

	ft2_text_file_watch_free(&watch);
	assert_null(watch.path);
}

/* The report: the file is written after the source is set up. */
static void test_watch_file_created_later(void **state)
{
	(void)state;
	struct ft2_text_file_watch watch = {0};
	uint64_t now = 100 * SECOND;

	remove(TEXT_PATH);
	assert_int_equal(ft2_text_file_watch_set(&watch, TEXT_PATH, false, now), FT2_TEXT_FILE_MISSING);
	assert_false(reload_within(&watch, &now, 3));

	write_utf8("now playing: two");
	expect_reload(&watch, &now, "now playing: two");

	ft2_text_file_watch_free(&watch);
}

/* A file that disappears for a moment, as the reporter's did, comes back. */
static void test_watch_file_deleted_and_recreated(void **state)
{
	(void)state;
	struct ft2_text_file_watch watch = {0};
	uint64_t now = 100 * SECOND;

	write_utf8("level 1");
	assert_int_equal(ft2_text_file_watch_set(&watch, TEXT_PATH, false, now), FT2_TEXT_FILE_LOAD);
	expect_reload(&watch, &now, "level 1");

	remove(TEXT_PATH);
	reload_within(&watch, &now, 3);

	write_utf8("level 2");
	expect_reload(&watch, &now, "level 2");

	ft2_text_file_watch_free(&watch);
}

/* Pointing the source at a file that does not exist yet stops it from
 * reading the previous one. */
static void test_watch_switch_to_missing_file(void **state)
{
	(void)state;
	struct ft2_text_file_watch watch = {0};
	uint64_t now = 100 * SECOND;

	write_utf8("old");
	assert_int_equal(ft2_text_file_watch_set(&watch, TEXT_PATH, false, now), FT2_TEXT_FILE_LOAD);

	assert_int_equal(ft2_text_file_watch_set(&watch, TEXT_PATH ".new", false, now), FT2_TEXT_FILE_MISSING);
	if (!watch.path || strcmp(watch.path, TEXT_PATH ".new") != 0) {
		fprintf(stderr, "the source still polls %s\n", watch.path ? watch.path : "nothing");
		fail();
	}

	assert_int_equal(ft2_text_file_watch_set(&watch, "", false, now), FT2_TEXT_FILE_MISSING);
	assert_null(watch.path);

	ft2_text_file_watch_free(&watch);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(test_utf8_file, teardown),
		cmocka_unit_test_teardown(test_utf16_file, teardown),
		cmocka_unit_test_teardown(test_utf16_odd_size, teardown),
		cmocka_unit_test_teardown(test_utf8_end, teardown),
		cmocka_unit_test_teardown(test_utf16_end_matches_utf8, teardown),
		cmocka_unit_test_teardown(test_utf8_nul_bytes, teardown),
		cmocka_unit_test_teardown(test_utf16_nul_units, teardown),
		cmocka_unit_test_teardown(test_watch_existing_file, teardown),
		cmocka_unit_test_teardown(test_watch_file_created_later, teardown),
		cmocka_unit_test_teardown(test_watch_file_deleted_and_recreated, teardown),
		cmocka_unit_test_teardown(test_watch_switch_to_missing_file, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
