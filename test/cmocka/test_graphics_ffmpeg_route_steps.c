#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "graphics/graphics-ffmpeg-route.h"

/* Characterization of the pixel route of ffmpeg_image_reformat_frame(),
 * moved from libobs/graphics/graphics-ffmpeg.c to
 * libobs/graphics/graphics-ffmpeg-route.c: which way a decoded frame goes
 * (copy, premultiplied rows, 16-bit rows or swscale), the premultiply, the
 * format swscale is given and its range, the format afterwards, the texture
 * format of that, and the straight row copy. No decoder and no FFmpeg header:
 * the test calls only the moved functions, which are hidden in libobs, so
 * this characterizes that C source. The copy's output is filled with a canary
 * first, so bytes it leaves unwritten show. */

#define CANARY 0xA5

/* AV_PIX_FMT_YUV420P10LE: a format the route does not name. */
#define UNNAMED_PIX_FMT 62

static const int alpha_modes[] = {
	GS_FFMPEG_IMAGE_ALPHA_STRAIGHT,
	GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY_SRGB,
	GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY,
};

/* What the swscale route and the premultiplied rows run for each mode. */
static const int premultiply_of[] = {
	GS_FFMPEG_IMAGE_PREMULTIPLY_NONE,
	GS_FFMPEG_IMAGE_PREMULTIPLY_SRGB,
	GS_FFMPEG_IMAGE_PREMULTIPLY_LINEAR,
};

static void check_plan(int pix_fmt, int alpha_mode, int route, int premultiply, int sws_format, bool full_range,
		       int out_format)
{
	const struct gs_ffmpeg_image_plan plan = gs_ffmpeg_image_plan(pix_fmt, alpha_mode);
	if (plan.route != route || plan.premultiply != premultiply || plan.sws_format != sws_format ||
	    plan.full_range != full_range || plan.out_format != out_format) {
		fprintf(stderr,
			"format %d, alpha mode %d: route %d premultiply %d sws %d full %d out %d, expected %d %d %d %d %d\n",
			pix_fmt, alpha_mode, plan.route, plan.premultiply, plan.sws_format, plan.full_range,
			plan.out_format, route, premultiply, sws_format, full_range, out_format);
		fail();
	}
}

/* The pixel formats are enum AVPixelFormat's values, the texture formats
 * enum gs_color_format's and the alpha modes enum gs_image_alpha_mode's
 * (graphics-ffmpeg.c checks them against FFmpeg and graphics.h). */
static void values(void **state)
{
	(void)state;
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_YUV420P, 0);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_YUV422P, 4);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_YUV444P, 5);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_YUV411P, 7);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ420P, 12);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ422P, 13);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ444P, 14);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_RGBA, 26);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_BGRA, 28);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_YUV440P, 31);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ440P, 32);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_RGBA64BE, 104);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_BGR0, 121);
	assert_int_equal(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ411P, 138);
	assert_int_equal(GS_FFMPEG_IMAGE_GS_RGBA, 3);
	assert_int_equal(GS_FFMPEG_IMAGE_GS_BGRX, 4);
	assert_int_equal(GS_FFMPEG_IMAGE_GS_BGRA, 5);
	assert_int_equal(GS_FFMPEG_IMAGE_GS_RGBA16, 7);
	assert_int_equal(GS_FFMPEG_IMAGE_ALPHA_STRAIGHT, 0);
	assert_int_equal(GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY_SRGB, 1);
	assert_int_equal(GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY, 2);
}

/* BGR0 is copied in every mode; RGBA and BGRA with straight alpha too. The
 * format stays. */
static void copy_route(void **state)
{
	(void)state;
	for (size_t m = 0; m < 3; m++)
		check_plan(GS_FFMPEG_IMAGE_PIX_FMT_BGR0, alpha_modes[m], GS_FFMPEG_IMAGE_ROUTE_COPY,
			   GS_FFMPEG_IMAGE_PREMULTIPLY_NONE, GS_FFMPEG_IMAGE_PIX_FMT_BGR0, false,
			   GS_FFMPEG_IMAGE_PIX_FMT_BGR0);
	check_plan(GS_FFMPEG_IMAGE_PIX_FMT_RGBA, GS_FFMPEG_IMAGE_ALPHA_STRAIGHT, GS_FFMPEG_IMAGE_ROUTE_COPY,
		   GS_FFMPEG_IMAGE_PREMULTIPLY_NONE, GS_FFMPEG_IMAGE_PIX_FMT_RGBA, false, GS_FFMPEG_IMAGE_PIX_FMT_RGBA);
	check_plan(GS_FFMPEG_IMAGE_PIX_FMT_BGRA, GS_FFMPEG_IMAGE_ALPHA_STRAIGHT, GS_FFMPEG_IMAGE_ROUTE_COPY,
		   GS_FFMPEG_IMAGE_PREMULTIPLY_NONE, GS_FFMPEG_IMAGE_PIX_FMT_BGRA, false, GS_FFMPEG_IMAGE_PIX_FMT_BGRA);
}

/* RGBA and BGRA premultiplied in sRGB or linearly go through the rows. */
static void premultiply_rows_route(void **state)
{
	(void)state;
	const int formats[] = {GS_FFMPEG_IMAGE_PIX_FMT_RGBA, GS_FFMPEG_IMAGE_PIX_FMT_BGRA};
	for (size_t f = 0; f < 2; f++) {
		check_plan(formats[f], GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY_SRGB, GS_FFMPEG_IMAGE_ROUTE_PREMULTIPLY_ROWS,
			   GS_FFMPEG_IMAGE_PREMULTIPLY_SRGB, formats[f], false, formats[f]);
		check_plan(formats[f], GS_FFMPEG_IMAGE_ALPHA_PREMULTIPLY, GS_FFMPEG_IMAGE_ROUTE_PREMULTIPLY_ROWS,
			   GS_FFMPEG_IMAGE_PREMULTIPLY_LINEAR, formats[f], false, formats[f]);
	}
}

/* RGBA64BE takes the 16-bit rows in every mode (they get the mode
 * themselves) and comes out RGBA. */
static void rgba64_route(void **state)
{
	(void)state;
	for (size_t m = 0; m < 3; m++)
		check_plan(GS_FFMPEG_IMAGE_PIX_FMT_RGBA64BE, alpha_modes[m], GS_FFMPEG_IMAGE_ROUTE_RGBA64,
			   GS_FFMPEG_IMAGE_PREMULTIPLY_NONE, GS_FFMPEG_IMAGE_PIX_FMT_RGBA64BE, false,
			   GS_FFMPEG_IMAGE_PIX_FMT_RGBA);
}

/* Every other format goes through swscale and comes out BGRA. The five
 * full-range YUVJ formats are given as their YUV twins with full range; the
 * rest as they are. */
static void sws_route(void **state)
{
	(void)state;
	static const int pairs[][3] = {
		{GS_FFMPEG_IMAGE_PIX_FMT_YUVJ420P, GS_FFMPEG_IMAGE_PIX_FMT_YUV420P, true},
		{GS_FFMPEG_IMAGE_PIX_FMT_YUVJ422P, GS_FFMPEG_IMAGE_PIX_FMT_YUV422P, true},
		{GS_FFMPEG_IMAGE_PIX_FMT_YUVJ444P, GS_FFMPEG_IMAGE_PIX_FMT_YUV444P, true},
		{GS_FFMPEG_IMAGE_PIX_FMT_YUVJ440P, GS_FFMPEG_IMAGE_PIX_FMT_YUV440P, true},
		{GS_FFMPEG_IMAGE_PIX_FMT_YUVJ411P, GS_FFMPEG_IMAGE_PIX_FMT_YUV411P, true},
		{GS_FFMPEG_IMAGE_PIX_FMT_YUV420P, GS_FFMPEG_IMAGE_PIX_FMT_YUV420P, false},
		{GS_FFMPEG_IMAGE_PIX_FMT_YUV422P, GS_FFMPEG_IMAGE_PIX_FMT_YUV422P, false},
		{GS_FFMPEG_IMAGE_PIX_FMT_YUV444P, GS_FFMPEG_IMAGE_PIX_FMT_YUV444P, false},
		{GS_FFMPEG_IMAGE_PIX_FMT_YUV440P, GS_FFMPEG_IMAGE_PIX_FMT_YUV440P, false},
		{GS_FFMPEG_IMAGE_PIX_FMT_YUV411P, GS_FFMPEG_IMAGE_PIX_FMT_YUV411P, false},
		{UNNAMED_PIX_FMT, UNNAMED_PIX_FMT, false},
		{-1, -1, false},
	};
	for (size_t p = 0; p < sizeof(pairs) / sizeof(pairs[0]); p++)
		for (size_t m = 0; m < 3; m++)
			check_plan(pairs[p][0], alpha_modes[m], GS_FFMPEG_IMAGE_ROUTE_SWS, premultiply_of[m],
				   pairs[p][1], pairs[p][2] != 0, GS_FFMPEG_IMAGE_PIX_FMT_BGRA);
}

/* convert_format(): RGBA, BGRA, BGR0 and RGBA64BE have their own texture
 * format; anything else is BGRX. */
static void color_formats(void **state)
{
	(void)state;
	assert_int_equal(gs_ffmpeg_image_color_format(GS_FFMPEG_IMAGE_PIX_FMT_RGBA), GS_FFMPEG_IMAGE_GS_RGBA);
	assert_int_equal(gs_ffmpeg_image_color_format(GS_FFMPEG_IMAGE_PIX_FMT_BGRA), GS_FFMPEG_IMAGE_GS_BGRA);
	assert_int_equal(gs_ffmpeg_image_color_format(GS_FFMPEG_IMAGE_PIX_FMT_BGR0), GS_FFMPEG_IMAGE_GS_BGRX);
	assert_int_equal(gs_ffmpeg_image_color_format(GS_FFMPEG_IMAGE_PIX_FMT_RGBA64BE), GS_FFMPEG_IMAGE_GS_RGBA16);
	assert_int_equal(gs_ffmpeg_image_color_format(GS_FFMPEG_IMAGE_PIX_FMT_YUVJ420P), GS_FFMPEG_IMAGE_GS_BGRX);
	assert_int_equal(gs_ffmpeg_image_color_format(GS_FFMPEG_IMAGE_PIX_FMT_YUV420P), GS_FFMPEG_IMAGE_GS_BGRX);
	assert_int_equal(gs_ffmpeg_image_color_format(UNNAMED_PIX_FMT), GS_FFMPEG_IMAGE_GS_BGRX);
	assert_int_equal(gs_ffmpeg_image_color_format(-1), GS_FFMPEG_IMAGE_GS_BGRX);
}

#define MAX_CX 9
#define MAX_CY 5
#define MAX_LINE (MAX_CX * 4 + 24)

static uint8_t src_byte(size_t y, size_t x)
{
	return (uint8_t)(y * 37 + x * 3 + 1);
}

/* Copies a cx x cy image from a source of the given line size into a
 * canary-filled buffer one image larger, and checks that each row has
 * min(cx * 4, linesize) bytes of its source line, the rest of the row and
 * everything after cx * cy * 4 the canary. */
static void check_copy(size_t cx, size_t cy, size_t linesize)
{
	uint8_t src[MAX_CY * MAX_LINE];
	uint8_t dst[2 * MAX_CX * MAX_CY * 4];
	for (size_t y = 0; y < MAX_CY; y++)
		for (size_t x = 0; x < MAX_LINE; x++)
			src[y * MAX_LINE + x] = 0;
	for (size_t y = 0; y < cy; y++)
		for (size_t x = 0; x < linesize; x++)
			src[y * linesize + x] = src_byte(y, x);
	memset(dst, CANARY, sizeof(dst));

	gs_ffmpeg_image_copy_rows(dst, src, cx, cy, linesize);

	const size_t row = cx * 4;
	const size_t copied = row < linesize ? row : linesize;
	for (size_t i = 0; i < sizeof(dst); i++) {
		const size_t y = i / row;
		const size_t x = i % row;
		const uint8_t expected = (y < cy && x < copied) ? src_byte(y, x) : CANARY;
		if (dst[i] != expected) {
			fprintf(stderr, "%zu x %zu, line size %zu: byte %zu is %u, expected %u\n", cx, cy, linesize, i,
				dst[i], expected);
			fail();
		}
	}
}

/* A source line size of cx * 4 is one copy of the whole image. */
static void copy_tight(void **state)
{
	(void)state;
	for (size_t cx = 1; cx <= MAX_CX; cx++)
		for (size_t cy = 1; cy <= MAX_CY; cy++)
			check_copy(cx, cy, cx * 4);
}

/* A padded source: each row's cx * 4 bytes, the padding never copied. */
static void copy_padded(void **state)
{
	(void)state;
	static const size_t pads[] = {1, 4, 8, 24};
	for (size_t cx = 1; cx <= MAX_CX; cx++)
		for (size_t cy = 1; cy <= MAX_CY; cy++)
			for (size_t p = 0; p < 4; p++)
				check_copy(cx, cy, cx * 4 + pads[p]);
}

/* A source line size shorter than cx * 4 copies only line size bytes into
 * each row and leaves the rest of the row unwritten (characterized, not
 * endorsed; FFmpeg never hands over such a frame). */
static void copy_short_linesize(void **state)
{
	(void)state;
	check_copy(4, 3, 12);
	check_copy(4, 3, 1);
	check_copy(9, 5, 31);
	check_copy(2, 5, 7);
}

/* No rows: nothing written, whatever the line size. */
static void copy_no_rows(void **state)
{
	(void)state;
	check_copy(4, 0, 16);
	check_copy(4, 0, 20);
	check_copy(4, 0, 3);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(values),       cmocka_unit_test(copy_route),  cmocka_unit_test(premultiply_rows_route),
		cmocka_unit_test(rgba64_route), cmocka_unit_test(sws_route),   cmocka_unit_test(color_formats),
		cmocka_unit_test(copy_tight),   cmocka_unit_test(copy_padded), cmocka_unit_test(copy_short_linesize),
		cmocka_unit_test(copy_no_rows),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
