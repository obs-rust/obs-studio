// The C++ headers go first: cmocka.h defines macros (fail, ...) that clash
// with libstdc++.
#include <string>

#include "utils/ImageFormat.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

/* obs-websocket's GetSourceScreenshot returns the picture as a data URL
 * whose MIME type comes from Utils::ImageFormat::MimeType() and the
 * requested QImageWriter format name (as GetVersion lists them). Prefixing
 * "image/" to the name gives "image/jpg" for "jpg", which is not a MIME
 * type, so clients and browsers that check it reject the picture. */

static void expect_mime(const char *format, const char *mime)
{
	std::string got = Utils::ImageFormat::MimeType(format);
	if (got != mime) {
		print_error("format '%s': got '%s', expected '%s'\n", format, got.c_str(), mime);
	}
	assert_string_equal(got.c_str(), mime);
}

static void test_jpg_is_jpeg(void **state)
{
	(void)state;
	expect_mime("jpg", "image/jpeg");
}

static void test_jfif_is_jpeg(void **state)
{
	(void)state;
	expect_mime("jfif", "image/jpeg");
}

static void test_ppm_is_portable_pixmap(void **state)
{
	(void)state;
	expect_mime("ppm", "image/x-portable-pixmap");
}

static void test_ico_is_icon(void **state)
{
	(void)state;
	expect_mime("ico", "image/vnd.microsoft.icon");
}

/* Guards: names that already are the MIME subtype stay as they were. */
static void test_unchanged_formats(void **state)
{
	(void)state;
	expect_mime("jpeg", "image/jpeg");
	expect_mime("png", "image/png");
	expect_mime("bmp", "image/bmp");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_jpg_is_jpeg),
		cmocka_unit_test(test_jfif_is_jpeg),
		cmocka_unit_test(test_ppm_is_portable_pixmap),
		cmocka_unit_test(test_ico_is_icon),
		cmocka_unit_test(test_unchanged_formats),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
