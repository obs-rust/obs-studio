#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "graphics/graphics-ffmpeg-orient.h"

/*
 * Characterization of the image orientation and size check of
 * libobs/graphics/graphics-ffmpeg.c, moved to
 * libobs/graphics/graphics-ffmpeg-orient.c before their Rust port (#2809):
 * - gs_ffmpeg_image_matrix_orientation(): the EXIF orientation from the signs
 *   of the display matrix (FFmpeg builds without the EXIF API);
 * - gs_ffmpeg_image_orient_needed(), _orient_swaps() and _orient_copy(): how
 *   ffmpeg_image_orient() turns the decoded RGBA image;
 * - gs_ffmpeg_image_too_large(): the size check against the device's
 *   maximum texture size.
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source into
 * itself and calls only them, through their header. graphics-ffmpeg.c keeps
 * the FFmpeg calls, the orientation read, the allocation and the warning.
 * The real-libobs tests (test_image_exif_orientation,
 * test_image_size_precheck, test_image_jpeg_range, test_graphics_ffmpeg_host)
 * cover the callers.
 *
 * The orientation comes from the image file: the display matrix, or (FFmpeg
 * builds without the EXIF API) atoi() of the "Orientation" metadata string,
 * so any int can reach _orient_needed() and _orient_copy().
 */

/* ------------------------------------------------------------------------ */
/* helpers */

/* The EXIF meaning of each orientation: where source pixel (x, y) of a
 * w x h image lands in the turned image, whose width is *dw. */
static void exif_position(int orient, size_t w, size_t h, size_t x, size_t y, size_t *dx, size_t *dy, size_t *dw)
{
	*dw = (orient >= 5 && orient <= 8) ? h : w;
	switch (orient) {
	case 2: /* mirror horizontal */
		*dx = w - 1 - x;
		*dy = y;
		break;
	case 3: /* rotate 180 */
		*dx = w - 1 - x;
		*dy = h - 1 - y;
		break;
	case 4: /* mirror vertical */
		*dx = x;
		*dy = h - 1 - y;
		break;
	case 5: /* transpose */
		*dx = y;
		*dy = x;
		break;
	case 6: /* rotate 90 clockwise */
		*dx = h - 1 - y;
		*dy = x;
		break;
	case 7: /* transverse */
		*dx = h - 1 - y;
		*dy = w - 1 - x;
		break;
	case 8: /* rotate 90 counter-clockwise */
		*dx = y;
		*dy = w - 1 - x;
		break;
	default:
		*dx = x;
		*dy = y;
	}
}

/* A w x h RGBA image whose every pixel holds its own index. */
static uint8_t *numbered(size_t w, size_t h)
{
	uint8_t *img = malloc(w * h * 4);
	for (size_t i = 0; i < w * h; i++) {
		img[i * 4 + 0] = (uint8_t)i;
		img[i * 4 + 1] = (uint8_t)(i >> 8);
		img[i * 4 + 2] = (uint8_t)(i >> 16);
		img[i * 4 + 3] = 0xa5;
	}
	return img;
}

/* Turns a w x h image and checks every pixel against the EXIF meaning. */
static void check_turn(int orient, size_t w, size_t h)
{
	uint8_t *src = numbered(w, h);
	uint8_t *dst = malloc(w * h * 4);
	memset(dst, 0xee, w * h * 4);

	gs_ffmpeg_image_orient_copy(dst, src, w, h, orient);

	for (size_t y = 0; y < h; y++) {
		for (size_t x = 0; x < w; x++) {
			size_t dx, dy, dw;
			exif_position(orient, w, h, x, y, &dx, &dy, &dw);
			if (memcmp(dst + (dx + dy * dw) * 4, src + (x + y * w) * 4, 4) != 0) {
				fprintf(stderr, "orientation %d, %zux%zu: pixel (%zu, %zu) not at (%zu, %zu)\n", orient,
					w, h, x, y, dx, dy);
				fail();
			}
		}
	}
	free(src);
	free(dst);
}

/* ------------------------------------------------------------------------ */
/* the display matrix */

/* Every combination of signs: the eight EXIF matrices give 1 to 8, the
 * others 0. Only the signs count, not the values. */
static void matrix_every_sign_combination(void **state)
{
	(void)state;
	static const int expect[8][4] = {
		{1, 0, 0, 1}, {-1, 0, 0, 1}, {-1, 0, 0, -1}, {1, 0, 0, -1},
		{0, 1, 1, 0}, {0, 1, -1, 0}, {0, -1, -1, 0}, {0, -1, 1, 0},
	};
	int found = 0;

	for (int a = -1; a <= 1; a++)
		for (int b = -1; b <= 1; b++)
			for (int c = -1; c <= 1; c++)
				for (int d = -1; d <= 1; d++) {
					int want = 0;
					for (int i = 0; i < 8; i++)
						if (a == expect[i][0] && b == expect[i][1] && c == expect[i][2] &&
						    d == expect[i][3])
							want = i + 1;
					found += want != 0;
					assert_int_equal(gs_ffmpeg_image_matrix_orientation(a, b, c, d), want);
					/* FFmpeg's matrices hold 16.16 fixed point values. */
					assert_int_equal(gs_ffmpeg_image_matrix_orientation(a * 65536, b * 65536,
											    c * 65536, d * 65536),
							 want);
				}
	assert_int_equal(found, 8);
}

static void matrix_extremes(void **state)
{
	(void)state;
	assert_int_equal(gs_ffmpeg_image_matrix_orientation(INT32_MAX, 0, 0, INT32_MAX), 1);
	assert_int_equal(gs_ffmpeg_image_matrix_orientation(INT32_MIN, 0, 0, INT32_MIN), 3);
	assert_int_equal(gs_ffmpeg_image_matrix_orientation(0, INT32_MIN, INT32_MAX, 0), 8);
	assert_int_equal(gs_ffmpeg_image_matrix_orientation(0, 0, 0, 0), 0);
	assert_int_equal(gs_ffmpeg_image_matrix_orientation(1, 1, 1, 1), 0);
}

/* ------------------------------------------------------------------------ */
/* the orientation tests */

static void needed_and_swaps(void **state)
{
	(void)state;
	for (int orient = -3; orient <= 12; orient++) {
		assert_int_equal(gs_ffmpeg_image_orient_needed(orient), orient != 0 && orient != 1);
		assert_int_equal(gs_ffmpeg_image_orient_swaps(orient), orient >= 5 && orient <= 8);
	}
	assert_true(gs_ffmpeg_image_orient_needed(INT32_MIN));
	assert_true(gs_ffmpeg_image_orient_needed(INT32_MAX));
	assert_false(gs_ffmpeg_image_orient_swaps(INT32_MAX));
}

/* ------------------------------------------------------------------------ */
/* the copy */

/* A 3 x 2 image, every orientation, spelled out:
 *   a b c
 *   d e f */
static void copy_small_image_spelled_out(void **state)
{
	(void)state;
	static const char *want[9] = {NULL, NULL, "cbafed", "fedcba", "defabc", "adbecf", "daebfc", "fcebda", "cfbead"};
	const uint8_t src[6 * 4] = {'a', 0, 0, 0, 'b', 0, 0, 0, 'c', 0, 0, 0, 'd', 0, 0, 0, 'e', 0, 0, 0, 'f', 0, 0, 0};

	for (int orient = 2; orient <= 8; orient++) {
		uint8_t dst[6 * 4];
		char got[7] = {0};
		gs_ffmpeg_image_orient_copy(dst, src, 3, 2, orient);
		for (int i = 0; i < 6; i++)
			got[i] = (char)dst[i * 4];
		if (strcmp(got, want[orient]) != 0) {
			fprintf(stderr, "orientation %d: %s, expected %s\n", orient, got, want[orient]);
			fail();
		}
	}
}

/* Every orientation from 2 to 8 on lines, odd sizes and sizes around the
 * 16-pixel tile, every pixel at its EXIF place. */
static void copy_every_size(void **state)
{
	(void)state;
	static const size_t sizes[] = {1, 2, 3, 7, 15, 16, 17, 31, 32, 33};
	for (int orient = 2; orient <= 8; orient++)
		for (size_t a = 0; a < sizeof(sizes) / sizeof(sizes[0]); a++)
			for (size_t b = 0; b < sizeof(sizes) / sizeof(sizes[0]); b++)
				check_turn(orient, sizes[a], sizes[b]);
}

/* Characterized, not endorsed: an orientation outside 2 to 8 (9 or more,
 * or below 0, which _orient_needed() lets through) writes every pixel to the
 * first one, in tile order, so the last pixel of the last tile is left
 * there; the rest of the output is not written. */
static void copy_unknown_orientation(void **state)
{
	(void)state;
	static const int unknown[] = {-1, 9, 42, INT32_MAX, INT32_MIN};
	for (size_t u = 0; u < sizeof(unknown) / sizeof(unknown[0]); u++) {
		const size_t w = 19, h = 5;
		uint8_t *src = numbered(w, h);
		uint8_t *dst = malloc(w * h * 4);
		memset(dst, 0xee, w * h * 4);

		gs_ffmpeg_image_orient_copy(dst, src, w, h, unknown[u]);

		assert_memory_equal(dst, src + (w * h - 1) * 4, 4);
		for (size_t i = 4; i < w * h * 4; i++)
			assert_int_equal(dst[i], 0xee);
		free(src);
		free(dst);
	}
}

/* ------------------------------------------------------------------------ */
/* the size check */

static void too_large_against_the_limit(void **state)
{
	(void)state;
	assert_false(gs_ffmpeg_image_too_large(0, 100000, 100000));
	assert_false(gs_ffmpeg_image_too_large(16384, 16384, 16384));
	assert_true(gs_ffmpeg_image_too_large(16384, 16385, 1));
	assert_true(gs_ffmpeg_image_too_large(16384, 1, 16385));
	assert_false(gs_ffmpeg_image_too_large(16384, 1, 1));
	assert_false(gs_ffmpeg_image_too_large(1, 1, 1));
	assert_true(gs_ffmpeg_image_too_large(1, 2, 1));
	/* A size not above 0 is left to the decoder. */
	assert_false(gs_ffmpeg_image_too_large(16384, 0, 100000));
	assert_false(gs_ffmpeg_image_too_large(16384, 100000, -1));
	assert_false(gs_ffmpeg_image_too_large(16384, INT32_MIN, INT32_MAX));
	assert_true(gs_ffmpeg_image_too_large(16384, INT32_MAX, INT32_MAX));
	assert_false(gs_ffmpeg_image_too_large(UINT32_MAX, INT32_MAX, INT32_MAX));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(matrix_every_sign_combination),
		cmocka_unit_test(matrix_extremes),
		cmocka_unit_test(needed_and_swaps),
		cmocka_unit_test(copy_small_image_spelled_out),
		cmocka_unit_test(copy_every_size),
		cmocka_unit_test(copy_unknown_orientation),
		cmocka_unit_test(too_large_against_the_limit),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
