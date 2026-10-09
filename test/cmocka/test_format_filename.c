#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <string.h>

#include <obs.h>
#include <util/bmem.h>
#include <util/platform.h>

/* platform.c uses this fixture instead of starting a video/graphics session. */
bool filename_test_get_video_info(struct obs_video_info *ovi)
{
	*ovi = (struct obs_video_info){0};
	return true;
}

static void check_filename(const char *format, const char *extension, bool space, const char *expected)
{
	char *filename = os_generate_formatted_filename(extension, space, format);
	assert_non_null(filename);
	assert_string_equal(filename, expected);
	bfree(filename);
}

static void filename_ascii_limit(void **state)
{
	UNUSED_PARAMETER(state);

	char format[257];
	char expected[256];
	memset(format, 'a', sizeof(format) - 1);
	format[sizeof(format) - 1] = '\0';
	memset(expected, 'a', sizeof(expected) - 1);
	expected[sizeof(expected) - 1] = '\0';

	check_filename(format, NULL, true, expected);
	check_filename(expected, NULL, true, expected);
	expected[254] = '\0';
	check_filename(expected, NULL, true, expected);
}

static void filename_preserves_extension(void **state)
{
	UNUSED_PARAMETER(state);

	static const char *const extensions[] = {"mkv", "ts", "tar.gz"};
	for (size_t i = 0; i < sizeof(extensions) / sizeof(extensions[0]); i++) {
		const char *extension = extensions[i];
		const size_t max_base = 255 - strlen(extension) - 1;
		for (size_t length = max_base - 1; length <= 300; length++) {
			char format[301] = {0};
			char expected[256] = {0};
			memset(format, 'a', length);
			memset(expected, 'a', length < max_base ? length : max_base);
			strcat(expected, ".");
			strcat(expected, extension);
			check_filename(format, extension, true, expected);
		}
	}
}

static void filename_short_utf8(void **state)
{
	UNUSED_PARAMETER(state);

	check_filename("caf\xc3\xa9 video", "mkv", true, "caf\xc3\xa9 video.mkv");
	check_filename("caf\xc3\xa9 video", "mkv", false, "caf\xc3\xa9_video.mkv");
}

static const char *const utf8_characters[] = {
	"\xc3\xa9",         /* U+00E9 */
	"\xe2\x80\x8b",     /* U+200B, as in obsproject/obs-studio#11211 */
	"\xf0\x9f\x8e\xa5", /* U+1F3A5 */
};

static void filename_utf8_crosses_limit(void **state)
{
	UNUSED_PARAMETER(state);

	static const char *const extensions[] = {NULL, "", "mkv"};
	for (size_t e = 0; e < sizeof(extensions) / sizeof(extensions[0]); e++) {
		const char *extension = extensions[e];
		const size_t max_base = extension && *extension ? 251 : 255;
		for (size_t i = 0; i < sizeof(utf8_characters) / sizeof(utf8_characters[0]); i++) {
			const char *character = utf8_characters[i];
			/* Include cuts just before and inside each UTF-8 character. */
			for (size_t kept_bytes = 0; kept_bytes < strlen(character); kept_bytes++) {
				char format[264] = {0};
				char expected[256] = {0};
				const size_t prefix_len = max_base - kept_bytes;
				memset(format, 'a', prefix_len);
				strcat(format, character);
				strcat(format, "tail");
				memset(expected, 'a', prefix_len);
				if (extension && *extension)
					strcat(expected, ".mkv");

				check_filename(format, extension, true, expected);
				if (!extension) {
					/* The same input with an extension has a smaller base budget. */
					expected[251] = '\0';
					strcat(expected, ".mkv");
					check_filename(format, "mkv", true, expected);
				}
			}
		}
	}
}

static void filename_utf8_fits_limit(void **state)
{
	UNUSED_PARAMETER(state);

	static const char *const extensions[] = {NULL, "", "mkv"};
	for (size_t e = 0; e < sizeof(extensions) / sizeof(extensions[0]); e++) {
		const char *extension = extensions[e];
		const size_t max_base = extension && *extension ? 251 : 255;
		for (size_t i = 0; i < sizeof(utf8_characters) / sizeof(utf8_characters[0]); i++) {
			char format[264] = {0};
			char expected[256] = {0};
			const size_t prefix_len = max_base - strlen(utf8_characters[i]);
			memset(format, 'a', prefix_len);
			strcat(format, utf8_characters[i]);
			strcpy(expected, format);
			if (extension && *extension)
				strcat(expected, ".mkv");
			check_filename(format, extension, true, expected);

			strcat(format, "tail");
			check_filename(format, extension, true, expected);
		}
	}
}

static void filename_invalid_utf8(void **state)
{
	UNUSED_PARAMETER(state);

	char format[301] = {0};
	char expected[256] = {0};
	memset(format, 0x80, 300);
	memset(expected, 0x80, 255);
	check_filename(format, NULL, true, expected);
	check_filename(format, "", true, expected);
	expected[251] = '\0';
	strcat(expected, ".mkv");
	check_filename(format, "mkv", true, expected);

	/* More than three continuation bytes at the cut must not eat the prefix. */
	memset(format, 'a', 251);
	memset(expected, 'a', 251);
	memset(expected + 251, 0x80, 4);
	check_filename(format, NULL, true, expected);
}

static void filename_long_extension(void **state)
{
	UNUSED_PARAMETER(state);

	/* An extension can leave fewer bytes than the first UTF-8 character needs. */
	for (size_t length = 251; length <= 254; length++) {
		char extension[255] = {0};
		char expected[256] = ".";
		memset(extension, 'e', length);
		strcat(expected, extension);
		check_filename("\xf0\x9f\x8e\xa5", extension, true, expected);
	}

	/* If the suffix alone exceeds 255 bytes, retain the legacy combined cap. */
	for (size_t length = 255; length <= 300; length++) {
		char extension[301] = {0};
		char expected[256] = "a.";
		memset(extension, 'e', length);
		memset(expected + 2, 'e', 253);
		check_filename("a", extension, true, expected);
	}
}

static void filename_base_ending(void **state)
{
	UNUSED_PARAMETER(state);

	/* A space or dot before the extension is not trailing in the final name. */
	static const char endings[] = {' ', '.'};
	for (size_t i = 0; i < sizeof(endings); i++) {
		char format[256] = {0};
		char expected[256] = {0};
		memset(format, 'a', 250);
		memset(expected, 'a', 250);
		format[250] = expected[250] = endings[i];
		strcat(format, "tail");
		strcat(expected, ".mkv");
		check_filename(format, "mkv", true, expected);
	}
	check_filename("subdir/clip", "mkv", true, "subdir/clip.mkv");
}

static void filename_zero_width_spaces(void **state)
{
	UNUSED_PARAMETER(state);

	/* The reported filename has 23 ASCII bytes followed by 100 zero-width spaces. */
	char format[327] = "2000-01-01 00-00-00 foo";
	char expected[256] = "2000-01-01 00-00-00 foo";
	for (size_t i = 0; i < 100; i++)
		strcat(format, "\xe2\x80\x8b");
	strcat(format, "bar");
	for (size_t i = 0; i < 77; i++)
		strcat(expected, "\xe2\x80\x8b");

	check_filename(format, NULL, true, expected);
	expected[251] = '\0';
	strcat(expected, ".mkv");
	check_filename(format, "mkv", true, expected);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(filename_ascii_limit),        cmocka_unit_test(filename_short_utf8),
		cmocka_unit_test(filename_utf8_crosses_limit), cmocka_unit_test(filename_utf8_fits_limit),
		cmocka_unit_test(filename_zero_width_spaces),  cmocka_unit_test(filename_preserves_extension),
		cmocka_unit_test(filename_invalid_utf8),       cmocka_unit_test(filename_long_extension),
		cmocka_unit_test(filename_base_ending),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
