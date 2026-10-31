#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "graphics/graphics-ffmpeg-rgba64.h"

/* Characterization of the 16-bit RGBA rows of ffmpeg_image_reformat_frame(),
 * moved from libobs/graphics/graphics-ffmpeg.c to
 * libobs/graphics/graphics-ffmpeg-rgba64.c: a big-endian RGBA64 frame (a
 * 16-bit PNG) to 8-bit RGBA, straight, premultiplied in sRGB or premultiplied.
 * No decoder and no FFmpeg header: the test calls only the moved function,
 * which is hidden in libobs, so this characterizes that C source. The output is
 * filled with a canary first, so bytes the function leaves unwritten show. */

#define CANARY 0xA5

static void put_texel(uint8_t *src, uint16_t r, uint16_t g, uint16_t b, uint16_t a)
{
	const uint16_t v[4] = {r, g, b, a};
	for (int c = 0; c < 4; c++) {
		src[2 * c] = (uint8_t)(v[c] >> 8);
		src[2 * c + 1] = (uint8_t)(v[c] & 0xFF);
	}
}

static void check_bytes(const char *what, const uint8_t *got, const uint8_t *expected, size_t n)
{
	for (size_t i = 0; i < n; i++) {
		if (got[i] != expected[i]) {
			fprintf(stderr, "%s: byte %zu is %u, expected %u\n", what, i, got[i], expected[i]);
			fail();
		}
	}
}

/* One texel of grey v with alpha a, in each mode: the colour byte and the
 * alpha byte. 33025 is 128.5 in 8 bits, which rounds up to 129: dividing by
 * 65536 instead of 65535 would give 128. */
struct grey {
	int mode;
	uint16_t v;
	uint16_t a;
	uint8_t colour;
	uint8_t alpha;
};

static const struct grey greys[] = {
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 0, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 0, 32768, 0, 128},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 0, 33025, 0, 129},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 0, 65535, 0, 255},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 1, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 1, 32768, 0, 128},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 1, 33025, 0, 129},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 1, 65535, 0, 255},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 257, 0, 1, 0},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 257, 32768, 1, 128},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 257, 33025, 1, 129},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 257, 65535, 1, 255},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 32767, 0, 127, 0},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 32767, 32768, 127, 128},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 32767, 33025, 127, 129},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 32767, 65535, 127, 255},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 32768, 0, 128, 0},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 32768, 32768, 128, 128},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 32768, 33025, 128, 129},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 32768, 65535, 128, 255},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 33025, 0, 129, 0},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 33025, 32768, 129, 128},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 33025, 33025, 129, 129},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 33025, 65535, 129, 255},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 65535, 0, 255, 0},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 65535, 32768, 255, 128},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 65535, 33025, 255, 129},
	{GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 65535, 65535, 255, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 0, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 0, 32768, 0, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 0, 33025, 0, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 0, 65535, 0, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 1, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 1, 32768, 0, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 1, 33025, 0, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 1, 65535, 0, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 257, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 257, 32768, 1, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 257, 33025, 1, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 257, 65535, 1, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 32767, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 32767, 32768, 92, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 32767, 33025, 92, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 32767, 65535, 127, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 32768, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 32768, 32768, 92, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 32768, 33025, 92, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 32768, 65535, 128, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 33025, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 33025, 32768, 93, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 33025, 33025, 93, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 33025, 65535, 129, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 65535, 0, 0, 0},
	/* Linear 0.5 is sRGB 0.735: 188. */
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 65535, 32768, 188, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 65535, 33025, 188, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 65535, 65535, 255, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 0, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 0, 32768, 0, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 0, 33025, 0, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 0, 65535, 0, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 1, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 1, 32768, 0, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 1, 33025, 0, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 1, 65535, 0, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 257, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 257, 32768, 1, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 257, 33025, 1, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 257, 65535, 1, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 32767, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 32767, 32768, 64, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 32767, 33025, 64, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 32767, 65535, 127, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 32768, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 32768, 32768, 64, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 32768, 33025, 64, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 32768, 65535, 128, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 33025, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 33025, 32768, 64, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 33025, 33025, 65, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 33025, 65535, 129, 255},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 65535, 0, 0, 0},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 65535, 32768, 128, 128},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 65535, 33025, 129, 129},
	{GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 65535, 65535, 255, 255},
};

static void check_greys(int mode)
{
	for (size_t i = 0; i < sizeof(greys) / sizeof(greys[0]); i++) {
		const struct grey *g = &greys[i];
		if (g->mode != mode)
			continue;

		uint8_t src[8];
		uint8_t dst[4];
		put_texel(src, g->v, g->v, g->v, g->a);
		memset(dst, CANARY, sizeof(dst));
		gs_ffmpeg_image_rgba64be_rows(dst, src, 1, 1, sizeof(src), mode);

		const uint8_t expected[4] = {g->colour, g->colour, g->colour, g->alpha};
		if (memcmp(dst, expected, sizeof(dst)) != 0) {
			fprintf(stderr, "mode %d, grey %u, alpha %u: %u %u %u %u, expected %u %u %u %u\n", mode, g->v,
				g->a, dst[0], dst[1], dst[2], dst[3], expected[0], expected[1], expected[2],
				expected[3]);
			fail();
		}
	}
}

static void straight_values(void **state)
{
	(void)state;
	check_greys(GS_FFMPEG_RGBA64_ALPHA_STRAIGHT);
}

static void premultiply_srgb_values(void **state)
{
	(void)state;
	check_greys(GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB);
}

static void premultiply_values(void **state)
{
	(void)state;
	check_greys(GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY);
}

/* The channels stay in order, and each is read big-endian. */
static void channel_order_and_byte_order(void **state)
{
	(void)state;
	uint8_t src[8];
	uint8_t dst[4];

	put_texel(src, 65535, 32768, 0, 65535);
	gs_ffmpeg_image_rgba64be_rows(dst, src, 1, 1, sizeof(src), GS_FFMPEG_RGBA64_ALPHA_STRAIGHT);
	const uint8_t in_order[4] = {255, 128, 0, 255};
	check_bytes("channel order", dst, in_order, 4);

	/* 0x00FF is 255 (1 in 8 bits), 0xFF00 is 65280 (254), 0x1234 is 4660
	 * (18). */
	const uint8_t be[8] = {0x00, 0xFF, 0xFF, 0x00, 0x12, 0x34, 0xFF, 0xFF};
	gs_ffmpeg_image_rgba64be_rows(dst, be, 1, 1, sizeof(be), GS_FFMPEG_RGBA64_ALPHA_STRAIGHT);
	const uint8_t from_be[4] = {1, 254, 18, 255};
	check_bytes("byte order", dst, from_be, 4);
}

/* A texel whose 8-bit value (straight) is its index: grey index * 257. */
static void fill_indexed(uint8_t *src, int cx, int cy, size_t linesize, uint8_t pad)
{
	memset(src, pad, linesize * (size_t)cy);
	for (int y = 0; y < cy; y++) {
		for (int x = 0; x < cx; x++) {
			const uint16_t v = (uint16_t)((y * cx + x) * 257);
			put_texel(src + (size_t)y * linesize + (size_t)x * 8, v, v, v, 65535);
		}
	}
}

static void check_indexed(const char *what, const uint8_t *dst, int texels)
{
	for (int i = 0; i < texels; i++) {
		const uint8_t expected[4] = {(uint8_t)i, (uint8_t)i, (uint8_t)i, 255};
		if (memcmp(dst + (size_t)i * 4, expected, 4) != 0) {
			fprintf(stderr, "%s: texel %d is %u %u %u %u, expected %u\n", what, i, dst[i * 4],
				dst[i * 4 + 1], dst[i * 4 + 2], dst[i * 4 + 3], (unsigned)i);
			fail();
		}
	}
}

static void one_by_one(void **state)
{
	(void)state;
	uint8_t src[8];
	uint8_t dst[8];
	fill_indexed(src, 1, 1, 8, 0);
	memset(dst, CANARY, sizeof(dst));
	gs_ffmpeg_image_rgba64be_rows(dst, src, 1, 1, 8, GS_FFMPEG_RGBA64_ALPHA_STRAIGHT);
	check_indexed("1 x 1", dst, 1);
	const uint8_t canary[4] = {CANARY, CANARY, CANARY, CANARY};
	check_bytes("past 1 x 1", dst + 4, canary, 4);
}

/* Odd widths, tight and with padding (filled with 0xEE, which no texel of the
 * image has): every texel in place, the padding never read, nothing written
 * past cx * cy * 4. */
static void odd_widths_and_padding(void **state)
{
	(void)state;
	static const int widths[] = {1, 3, 5, 7, 9};
	static const size_t pads[] = {0, 2, 8, 24};
	uint8_t src[9 * 8 * 5 + 24 * 5];
	uint8_t dst[9 * 5 * 4 + 16];

	for (size_t w = 0; w < sizeof(widths) / sizeof(widths[0]); w++) {
		for (size_t p = 0; p < sizeof(pads) / sizeof(pads[0]); p++) {
			for (int mode = 0; mode < 3; mode++) {
				const int cx = widths[w];
				const int cy = 5;
				const size_t linesize = (size_t)cx * 8 + pads[p];
				fill_indexed(src, cx, cy, linesize, 0xEE);
				memset(dst, CANARY, sizeof(dst));
				gs_ffmpeg_image_rgba64be_rows(dst, src, cx, cy, linesize, mode);

				char what[64];
				snprintf(what, sizeof(what), "%d x %d, padding %zu, mode %d", cx, cy, pads[p], mode);
				/* Alpha is 1, so every mode gives the straight values. */
				check_indexed(what, dst, cx * cy);
				for (size_t i = (size_t)(cx * cy * 4); i < sizeof(dst); i++) {
					if (dst[i] != CANARY) {
						fprintf(stderr, "%s: byte %zu written\n", what, i);
						fail();
					}
				}
			}
		}
	}
}

/* Characterized, not endorsed: a source line size shorter than cx * 8 bytes
 * converts only line size / 8 texels per row, and the destination advances by
 * what was written, so the rows pack together and the end of the output is
 * left unwritten. The source is not stepped to the next line either (the step
 * is line size minus the bytes read, which is then the line size itself), so
 * every row reads on from where the last one stopped: the texels come from the
 * source as one run of 8 bytes each, whatever the line size's remainder.
 * FFmpeg never hands over such a frame. */
static void check_short_linesize(size_t linesize)
{
	const int cx = 4;
	const int cy = 3;
	const int per_row = (int)(linesize / 8);
	uint8_t src[3 * 31];
	uint8_t dst[4 * 3 * 4];

	memset(src, 0xEE, sizeof(src));
	for (int k = 0; k < per_row * cy; k++) {
		const uint16_t v = (uint16_t)(k * 257);
		put_texel(src + (size_t)k * 8, v, v, v, 65535);
	}
	memset(dst, CANARY, sizeof(dst));
	gs_ffmpeg_image_rgba64be_rows(dst, src, cx, cy, linesize, GS_FFMPEG_RGBA64_ALPHA_STRAIGHT);

	char what[48];
	snprintf(what, sizeof(what), "line size %zu", linesize);
	check_indexed(what, dst, per_row * cy);
	for (size_t i = (size_t)(per_row * cy * 4); i < sizeof(dst); i++) {
		if (dst[i] != CANARY) {
			fprintf(stderr, "%s: byte %zu written\n", what, i);
			fail();
		}
	}
}

static void short_linesize_packs_rows(void **state)
{
	(void)state;
	check_short_linesize(16); /* 2 texels */
	check_short_linesize(20); /* 2 texels and 4 bytes, not skipped */
	check_short_linesize(31); /* 3 texels and 7 bytes, not skipped */
	check_short_linesize(7);  /* under one texel: nothing */
}

/* Characterized, not endorsed: an alpha mode outside the three writes
 * nothing. */
static void unknown_alpha_mode_writes_nothing(void **state)
{
	(void)state;
	static const int modes[] = {-1, 3, 100};
	uint8_t src[2 * 2 * 8];
	uint8_t dst[2 * 2 * 4];
	fill_indexed(src, 2, 2, 16, 0);

	for (size_t m = 0; m < sizeof(modes) / sizeof(modes[0]); m++) {
		memset(dst, CANARY, sizeof(dst));
		gs_ffmpeg_image_rgba64be_rows(dst, src, 2, 2, 16, modes[m]);
		for (size_t i = 0; i < sizeof(dst); i++)
			assert_int_equal(dst[i], CANARY);
	}
}

/* No rows or no columns: nothing is written. */
static void empty_image_writes_nothing(void **state)
{
	(void)state;
	/* Two rows of 16 bytes and one to spare: with no columns the source still
	 * steps over each row's line size. */
	uint8_t src[48];
	uint8_t dst[16];
	fill_indexed(src, 2, 3, 16, 0);

	for (int mode = 0; mode < 3; mode++) {
		memset(dst, CANARY, sizeof(dst));
		gs_ffmpeg_image_rgba64be_rows(dst, src, 2, 0, 16, mode);
		gs_ffmpeg_image_rgba64be_rows(dst, src, 0, 2, 16, mode);
		for (size_t i = 0; i < sizeof(dst); i++)
			assert_int_equal(dst[i], CANARY);
	}
}

/* The alpha modes are enum gs_image_alpha_mode's values. */
static void alpha_mode_values(void **state)
{
	(void)state;
	assert_int_equal(GS_FFMPEG_RGBA64_ALPHA_STRAIGHT, 0);
	assert_int_equal(GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY_SRGB, 1);
	assert_int_equal(GS_FFMPEG_RGBA64_ALPHA_PREMULTIPLY, 2);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(alpha_mode_values),
		cmocka_unit_test(straight_values),
		cmocka_unit_test(premultiply_srgb_values),
		cmocka_unit_test(premultiply_values),
		cmocka_unit_test(channel_order_and_byte_order),
		cmocka_unit_test(one_by_one),
		cmocka_unit_test(odd_widths_and_padding),
		cmocka_unit_test(short_linesize_packs_rows),
		cmocka_unit_test(unknown_alpha_mode_writes_nothing),
		cmocka_unit_test(empty_image_writes_nothing),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
