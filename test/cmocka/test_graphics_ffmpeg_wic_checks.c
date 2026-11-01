#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <string.h>
#include <cmocka.h>

#include "libobs/graphics/graphics-ffmpeg-host-wic-checks.h"

/*
 * Characterization of two checks of the image loader's Windows host
 * (graphics-ffmpeg-host-windows.c), moved to graphics-ffmpeg-host-wic-checks.c:
 * the file-name check at the top of gs_image_host_fallback_load() and the
 * pixel-format pick of wic_image_init_internal(). The file includes no
 * Windows header, so this runs on every platform.
 *
 * The test includes only the header: test_graphics_ffmpeg_wic_checks builds
 * the C file beside it, so a build can link other definitions of the same two
 * functions instead.
 *
 * Only names of 4 bytes or more are passed: for a shorter one the C compares
 * from before the start of the name.
 */

static bool refuses(const char *file)
{
	return gs_image_host_fallback_refuses(file, strlen(file));
}

/* A name of exactly 4 bytes is refused unless it is ".jxr", in any case. */
static void test_four_bytes(void **state)
{
	(void)state;

	assert_false(refuses(".jxr"));
	assert_false(refuses(".JXR"));
	assert_false(refuses(".JxR"));
	assert_false(refuses(".jXr"));

	assert_true(refuses("a.jx"));
	assert_true(refuses("abcd"));
	assert_true(refuses("jxr."));
	assert_true(refuses("jxr "));
	assert_true(refuses(",jxr"));
	assert_true(refuses(".jxs"));
	assert_true(refuses(".png"));
}

/* Characterized, not endorsed: a name longer than 4 bytes is never refused,
 * whatever it ends with. WIC then refuses any pixel format but the two
 * below. */
static void test_longer_names(void **state)
{
	(void)state;

	assert_false(refuses("a.jxr"));
	assert_false(refuses("a.JXR"));
	assert_false(refuses("a.png"));
	assert_false(refuses("a.jxra"));
	assert_false(refuses("abcde"));
	assert_false(refuses("C:\\images\\hdr.jxr"));
	assert_false(refuses("C:\\images\\shot.png"));
	assert_false(refuses("C:\\images\\shot.wdp"));
	assert_false(refuses("/images/no-extension"));
}

/* GUID_WICPixelFormat64bppRGBAHalf (scrgb) is copied, and
 * GUID_WICPixelFormat32bppBGR101010 (pq10) converted; pq10 wins if both
 * are set, which two different GUIDs never are. */
static void test_pick(void **state)
{
	(void)state;

	assert_int_equal(gs_image_wic_kind(false, false), GS_IMAGE_WIC_UNSUPPORTED);
	assert_int_equal(gs_image_wic_kind(true, false), GS_IMAGE_WIC_COPY);
	assert_int_equal(gs_image_wic_kind(false, true), GS_IMAGE_WIC_PQ10);
	assert_int_equal(gs_image_wic_kind(true, true), GS_IMAGE_WIC_PQ10);
}

/* The values a Rust shim must return. */
static void test_kind_values(void **state)
{
	(void)state;

	assert_int_equal(GS_IMAGE_WIC_UNSUPPORTED, 0);
	assert_int_equal(GS_IMAGE_WIC_COPY, 1);
	assert_int_equal(GS_IMAGE_WIC_PQ10, 2);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_four_bytes),
		cmocka_unit_test(test_longer_names),
		cmocka_unit_test(test_pick),
		cmocka_unit_test(test_kind_values),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
