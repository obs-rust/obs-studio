#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include <util/bmem.h>
#include <media-io/format-conversion.h>

/* Characterization of the exported functions in media-io/format-conversion.c.
 * Nothing in the tree calls them, so this is their only C coverage.
 *
 * Every buffer is bmalloc'd (32-byte aligned) and starts filled with CANARY.
 * Each test builds the whole expected buffer, canary included, and compares
 * all of it, so a write outside the expected area fails too.
 *
 * The UYVX functions use aligned 16-byte SSE2 loads and step 4 pixels and 2
 * rows at a time, so their inputs keep in_linesize a multiple of 16, widths a
 * multiple of 4 and even row ranges. The decompress_* functions access whole
 * uint16_t/uint32_t values, so their linesizes keep that alignment. */

#define CANARY 0xA5
/* Extra canary bytes after every buffer. */
#define SLACK 16

static uint8_t *canary_buf(size_t size)
{
	uint8_t *buf = bmalloc(size + SLACK);
	memset(buf, CANARY, size + SLACK);
	return buf;
}

static void assert_buf_equal(const uint8_t *actual, const uint8_t *expected, size_t size)
{
	assert_memory_equal(actual, expected, size + SLACK);
}

static void put32(uint8_t *buf, size_t offset, uint32_t value)
{
	memcpy(buf + offset, &value, sizeof(value));
}

/* Test image channels, distinct per channel and position. */
typedef uint8_t (*channel_fn)(uint32_t x, uint32_t y);

static uint8_t chan_u(uint32_t x, uint32_t y)
{
	return (uint8_t)(0x10 + 3 * x + 7 * y);
}

static uint8_t chan_y(uint32_t x, uint32_t y)
{
	return (uint8_t)(0x80 + 5 * x + 11 * y);
}

static uint8_t chan_v(uint32_t x, uint32_t y)
{
	return (uint8_t)(0xC0 + 13 * x + 2 * y);
}

/* The chroma average of the 2x2 block at (x, y), rounded down. */
static uint8_t avg4(channel_fn chan, uint32_t x, uint32_t y)
{
	return (uint8_t)((chan(x, y) + chan(x + 1, y) + chan(x, y + 1) + chan(x + 1, y + 1)) >> 2);
}

/* ------------------------------------------------------------------------ */
/* UYVX: one 32-bit pixel per 4 bytes, U Y V X in memory order.              */

static uint8_t *make_uyvx(uint32_t width, uint32_t height, uint32_t linesize)
{
	uint8_t *buf = canary_buf((size_t)linesize * height);
	for (uint32_t y = 0; y < height; y++) {
		for (uint32_t x = 0; x < width; x++) {
			uint8_t *px = buf + (size_t)y * linesize + x * 4;
			px[0] = chan_u(x, y);
			px[1] = chan_y(x, y);
			px[2] = chan_v(x, y);
			px[3] = 0xEE;
		}
	}
	return buf;
}

/* Luma for rows [start_y, end_y) and columns [0, width). */
static void expect_luma(uint8_t *lum, uint32_t linesize, uint32_t width, uint32_t start_y, uint32_t end_y)
{
	for (uint32_t y = start_y; y < end_y; y++)
		for (uint32_t x = 0; x < width; x++)
			lum[(size_t)y * linesize + x] = chan_y(x, y);
}

#define W 8
#define H 4
#define UYVX_LINESIZE (W * 4)

static void test_i420(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize[3] = {W, W / 2, W / 2};
	uint8_t *input = make_uyvx(W, H, UYVX_LINESIZE);
	uint8_t *output[3] = {canary_buf(W * H), canary_buf(W * H / 4), canary_buf(W * H / 4)};
	uint8_t *expected[3] = {canary_buf(W * H), canary_buf(W * H / 4), canary_buf(W * H / 4)};

	expect_luma(expected[0], W, W, 0, H);
	for (uint32_t cy = 0; cy < H / 2; cy++) {
		for (uint32_t cx = 0; cx < W / 2; cx++) {
			expected[1][cy * (W / 2) + cx] = avg4(chan_u, cx * 2, cy * 2);
			expected[2][cy * (W / 2) + cx] = avg4(chan_v, cx * 2, cy * 2);
		}
	}

	compress_uyvx_to_i420(input, UYVX_LINESIZE, 0, H, output, out_linesize);

	assert_buf_equal(output[0], expected[0], W * H);
	assert_buf_equal(output[1], expected[1], W * H / 4);
	assert_buf_equal(output[2], expected[2], W * H / 4);

	bfree(input);
	for (size_t i = 0; i < 3; i++) {
		bfree(output[i]);
		bfree(expected[i]);
	}
}

/* Chroma is (a + b + c + d) >> 2 over each 2x2 block. */
static void test_i420_chroma_rounds_down(void **state)
{
	UNUSED_PARAMETER(state);

	/* 4x2 pixels, U Y V X each. */
	uint8_t *input = canary_buf(32);
	static const uint8_t pixels[32] = {
		1, 0x10, 10, 0, 2, 0x11, 20, 0, 3, 0x12, 30, 0, 4, 0x13, 40, 0, /* row 0 */
		5, 0x14, 50, 0, 6, 0x15, 61, 0, 7, 0x16, 70, 0, 8, 0x17, 80, 0, /* row 1 */
	};
	memcpy(input, pixels, sizeof(pixels));

	/* The width is min(in_linesize, out_linesize[0]) = 4 pixels. */
	const uint32_t out_linesize[3] = {4, 4, 4};
	uint8_t *output[3] = {canary_buf(8), canary_buf(4), canary_buf(4)};

	compress_uyvx_to_i420(input, 16, 0, 2, output, out_linesize);

	static const uint8_t lum[4] = {0x10, 0x11, 0x12, 0x13};
	static const uint8_t lum1[4] = {0x14, 0x15, 0x16, 0x17};
	assert_memory_equal(output[0], lum, 4);
	assert_memory_equal(output[0] + 4, lum1, 4);
	assert_int_equal(output[0][8], CANARY);
	/* U: (1 + 2 + 5 + 6) >> 2 = 3, (3 + 4 + 7 + 8) >> 2 = 5.
	 * V: (10 + 20 + 50 + 61) >> 2 = 35, (30 + 40 + 70 + 80) >> 2 = 55. */
	assert_int_equal(output[1][0], 3);
	assert_int_equal(output[1][1], 5);
	assert_int_equal(output[2][0], 35);
	assert_int_equal(output[2][1], 55);
	assert_int_equal(output[1][2], CANARY);
	assert_int_equal(output[2][2], CANARY);

	bfree(input);
	for (size_t i = 0; i < 3; i++)
		bfree(output[i]);
}

static void test_i420_rows(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize[3] = {W, W / 2, W / 2};
	uint8_t *input = make_uyvx(W, H, UYVX_LINESIZE);
	uint8_t *output[3] = {canary_buf(W * H), canary_buf(W * H / 4), canary_buf(W * H / 4)};
	uint8_t *expected[3] = {canary_buf(W * H), canary_buf(W * H / 4), canary_buf(W * H / 4)};

	/* Rows 2 and 3 only: chroma row 1. */
	expect_luma(expected[0], W, W, 2, 4);
	for (uint32_t cx = 0; cx < W / 2; cx++) {
		expected[1][W / 2 + cx] = avg4(chan_u, cx * 2, 2);
		expected[2][W / 2 + cx] = avg4(chan_v, cx * 2, 2);
	}

	compress_uyvx_to_i420(input, UYVX_LINESIZE, 2, 4, output, out_linesize);

	assert_buf_equal(output[0], expected[0], W * H);
	assert_buf_equal(output[1], expected[1], W * H / 4);
	assert_buf_equal(output[2], expected[2], W * H / 4);

	bfree(input);
	for (size_t i = 0; i < 3; i++) {
		bfree(output[i]);
		bfree(expected[i]);
	}
}

/* The width is min(in_linesize, out_linesize[0]) pixels. */
static void test_i420_out_linesize_limits_width(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize[3] = {4, 2, 2};
	uint8_t *input = make_uyvx(W, H, UYVX_LINESIZE);
	uint8_t *output[3] = {canary_buf(4 * H), canary_buf(2 * H / 2), canary_buf(2 * H / 2)};
	uint8_t *expected[3] = {canary_buf(4 * H), canary_buf(2 * H / 2), canary_buf(2 * H / 2)};

	expect_luma(expected[0], 4, 4, 0, H);
	for (uint32_t cy = 0; cy < H / 2; cy++) {
		for (uint32_t cx = 0; cx < 2; cx++) {
			expected[1][cy * 2 + cx] = avg4(chan_u, cx * 2, cy * 2);
			expected[2][cy * 2 + cx] = avg4(chan_v, cx * 2, cy * 2);
		}
	}

	compress_uyvx_to_i420(input, UYVX_LINESIZE, 0, H, output, out_linesize);

	assert_buf_equal(output[0], expected[0], 4 * H);
	assert_buf_equal(output[1], expected[1], 2 * H / 2);
	assert_buf_equal(output[2], expected[2], 2 * H / 2);

	bfree(input);
	for (size_t i = 0; i < 3; i++) {
		bfree(output[i]);
		bfree(expected[i]);
	}
}

/* The V plane is addressed with out_linesize[1]; out_linesize[2] is unused. */
static void test_i420_v_plane_uses_u_linesize(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize[3] = {W, W / 2, W};
	uint8_t *input = make_uyvx(W, H, UYVX_LINESIZE);
	uint8_t *output[3] = {canary_buf(W * H), canary_buf(W * H / 4), canary_buf(W * H / 2)};
	uint8_t *expected = canary_buf(W * H / 2);

	for (uint32_t cy = 0; cy < H / 2; cy++)
		for (uint32_t cx = 0; cx < W / 2; cx++)
			expected[cy * (W / 2) + cx] = avg4(chan_v, cx * 2, cy * 2);

	compress_uyvx_to_i420(input, UYVX_LINESIZE, 0, H, output, out_linesize);

	assert_buf_equal(output[2], expected, W * H / 2);

	bfree(input);
	bfree(expected);
	for (size_t i = 0; i < 3; i++)
		bfree(output[i]);
}

static void test_nv12(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize[2] = {W, W};
	uint8_t *input = make_uyvx(W, H, UYVX_LINESIZE);
	uint8_t *output[2] = {canary_buf(W * H), canary_buf(W * H / 2)};
	uint8_t *expected[2] = {canary_buf(W * H), canary_buf(W * H / 2)};

	/* Chroma is interleaved U V per 2x2 block. */
	expect_luma(expected[0], W, W, 0, H);
	for (uint32_t cy = 0; cy < H / 2; cy++) {
		for (uint32_t cx = 0; cx < W / 2; cx++) {
			expected[1][cy * W + cx * 2] = avg4(chan_u, cx * 2, cy * 2);
			expected[1][cy * W + cx * 2 + 1] = avg4(chan_v, cx * 2, cy * 2);
		}
	}

	compress_uyvx_to_nv12(input, UYVX_LINESIZE, 0, H, output, out_linesize);

	assert_buf_equal(output[0], expected[0], W * H);
	assert_buf_equal(output[1], expected[1], W * H / 2);

	bfree(input);
	for (size_t i = 0; i < 2; i++) {
		bfree(output[i]);
		bfree(expected[i]);
	}
}

static void test_nv12_rows_and_linesize(void **state)
{
	UNUSED_PARAMETER(state);

	/* A padded chroma plane; rows 2 and 3 only. */
	const uint32_t out_linesize[2] = {W, W + 8};
	uint8_t *input = make_uyvx(W, H, UYVX_LINESIZE);
	uint8_t *output[2] = {canary_buf(W * H), canary_buf((W + 8) * H / 2)};
	uint8_t *expected[2] = {canary_buf(W * H), canary_buf((W + 8) * H / 2)};

	expect_luma(expected[0], W, W, 2, 4);
	for (uint32_t cx = 0; cx < W / 2; cx++) {
		expected[1][(W + 8) + cx * 2] = avg4(chan_u, cx * 2, 2);
		expected[1][(W + 8) + cx * 2 + 1] = avg4(chan_v, cx * 2, 2);
	}

	compress_uyvx_to_nv12(input, UYVX_LINESIZE, 2, 4, output, out_linesize);

	assert_buf_equal(output[0], expected[0], W * H);
	assert_buf_equal(output[1], expected[1], (W + 8) * H / 2);

	bfree(input);
	for (size_t i = 0; i < 2; i++) {
		bfree(output[i]);
		bfree(expected[i]);
	}
}

static void test_i444(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize[3] = {W, W, W};
	uint8_t *input = make_uyvx(W, H, UYVX_LINESIZE);
	uint8_t *output[3] = {canary_buf(W * H), canary_buf(W * H), canary_buf(W * H)};
	uint8_t *expected[3] = {canary_buf(W * H), canary_buf(W * H), canary_buf(W * H)};

	expect_luma(expected[0], W, W, 0, H);
	for (uint32_t y = 0; y < H; y++) {
		for (uint32_t x = 0; x < W; x++) {
			expected[1][y * W + x] = chan_u(x, y);
			expected[2][y * W + x] = chan_v(x, y);
		}
	}

	convert_uyvx_to_i444(input, UYVX_LINESIZE, 0, H, output, out_linesize);

	for (size_t i = 0; i < 3; i++)
		assert_buf_equal(output[i], expected[i], W * H);

	bfree(input);
	for (size_t i = 0; i < 3; i++) {
		bfree(output[i]);
		bfree(expected[i]);
	}
}

/* All three planes are addressed with out_linesize[0]; [1] and [2] are
 * unused. */
static void test_i444_planes_use_luma_linesize(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize[3] = {W, 1, 2};
	const size_t size = W * H;
	uint8_t *input = make_uyvx(W, H, UYVX_LINESIZE);
	uint8_t *output[3] = {canary_buf(size), canary_buf(size), canary_buf(size)};
	uint8_t *expected[3] = {canary_buf(size), canary_buf(size), canary_buf(size)};

	/* Rows 2 and 3 only. */
	expect_luma(expected[0], W, W, 2, 4);
	for (uint32_t y = 2; y < 4; y++) {
		for (uint32_t x = 0; x < W; x++) {
			expected[1][y * W + x] = chan_u(x, y);
			expected[2][y * W + x] = chan_v(x, y);
		}
	}

	convert_uyvx_to_i444(input, UYVX_LINESIZE, 2, 4, output, out_linesize);

	for (size_t i = 0; i < 3; i++)
		assert_buf_equal(output[i], expected[i], size);

	bfree(input);
	for (size_t i = 0; i < 3; i++) {
		bfree(output[i]);
		bfree(expected[i]);
	}
}

/* ------------------------------------------------------------------------ */
/* decompress_*: planar or packed YUV to one native-endian uint32_t per pixel. */

static uint8_t *make_plane(channel_fn chan, uint32_t width, uint32_t height, uint32_t linesize)
{
	uint8_t *buf = canary_buf((size_t)linesize * height);
	for (uint32_t y = 0; y < height; y++)
		for (uint32_t x = 0; x < width; x++)
			buf[(size_t)y * linesize + x] = chan(x, y);
	return buf;
}

/* Output is Y << 16 | U << 8 | V, with U and V shared by each 2x2 block. */
static uint32_t i420_pixel(uint32_t x, uint32_t y)
{
	return (uint32_t)chan_y(x, y) << 16 | (uint32_t)chan_u(x / 2, y / 2) << 8 | chan_v(x / 2, y / 2);
}

static void test_decompress_420(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize = W * 4 + 8;
	const size_t out_size = (size_t)out_linesize * H;
	/* The chroma planes have their own, padded, linesizes. */
	const uint32_t in_linesize[3] = {W, W / 2 + 2, W / 2 + 4};
	uint8_t *planes[3] = {
		make_plane(chan_y, W, H, in_linesize[0]),
		make_plane(chan_u, W / 2, H / 2, in_linesize[1]),
		make_plane(chan_v, W / 2, H / 2, in_linesize[2]),
	};
	const uint8_t *const input[3] = {planes[0], planes[1], planes[2]};
	uint8_t *output = canary_buf(out_size);
	uint8_t *expected = canary_buf(out_size);

	for (uint32_t y = 0; y < H; y++)
		for (uint32_t x = 0; x < W; x++)
			put32(expected, (size_t)y * out_linesize + x * 4, i420_pixel(x, y));

	decompress_420(input, in_linesize, 0, H, output, out_linesize);

	assert_buf_equal(output, expected, out_size);

	for (size_t i = 0; i < 3; i++)
		bfree(planes[i]);
	bfree(output);
	bfree(expected);
}

/* Rows are processed in pairs from start_y / 2 to end_y / 2, both rounded
 * down: start_y 1, end_y 3 writes rows 0 and 1, not 1 and 2. */
static void test_decompress_420_odd_rows_round_down(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize = W * 4;
	const size_t out_size = (size_t)out_linesize * H;
	const uint32_t in_linesize[3] = {W, W / 2, W / 2};
	uint8_t *planes[3] = {
		make_plane(chan_y, W, H, in_linesize[0]),
		make_plane(chan_u, W / 2, H / 2, in_linesize[1]),
		make_plane(chan_v, W / 2, H / 2, in_linesize[2]),
	};
	const uint8_t *const input[3] = {planes[0], planes[1], planes[2]};
	uint8_t *output = canary_buf(out_size);
	uint8_t *expected = canary_buf(out_size);

	for (uint32_t y = 0; y < 2; y++)
		for (uint32_t x = 0; x < W; x++)
			put32(expected, (size_t)y * out_linesize + x * 4, i420_pixel(x, y));

	decompress_420(input, in_linesize, 1, 3, output, out_linesize);
	assert_buf_equal(output, expected, out_size);

	/* start_y 2, end_y 3: 1 / 1, nothing. */
	decompress_420(input, in_linesize, 2, 3, output, out_linesize);
	assert_buf_equal(output, expected, out_size);

	for (size_t i = 0; i < 3; i++)
		bfree(planes[i]);
	bfree(output);
	bfree(expected);
}

/* The width is in_linesize[0] rounded down to even, whatever out_linesize
 * is. */
static void test_decompress_420_width_from_luma_linesize(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize = W * 4;
	const size_t out_size = (size_t)out_linesize * 2;
	const uint32_t in_linesize[3] = {7, 4, 4};
	uint8_t *planes[3] = {
		make_plane(chan_y, 7, 2, in_linesize[0]),
		make_plane(chan_u, 4, 1, in_linesize[1]),
		make_plane(chan_v, 4, 1, in_linesize[2]),
	};
	const uint8_t *const input[3] = {planes[0], planes[1], planes[2]};
	uint8_t *output = canary_buf(out_size);
	uint8_t *expected = canary_buf(out_size);

	for (uint32_t y = 0; y < 2; y++)
		for (uint32_t x = 0; x < 6; x++)
			put32(expected, (size_t)y * out_linesize + x * 4, i420_pixel(x, y));

	decompress_420(input, in_linesize, 0, 2, output, out_linesize);

	assert_buf_equal(output, expected, out_size);

	for (size_t i = 0; i < 3; i++)
		bfree(planes[i]);
	bfree(output);
	bfree(expected);
}

/* Interleaved U V bytes for an NV12 chroma plane. */
static uint8_t *make_uv_plane(uint32_t width, uint32_t height, uint32_t linesize)
{
	uint8_t *buf = canary_buf((size_t)linesize * height);
	for (uint32_t y = 0; y < height; y++) {
		for (uint32_t x = 0; x < width; x++) {
			buf[(size_t)y * linesize + x * 2] = chan_u(x, y);
			buf[(size_t)y * linesize + x * 2 + 1] = chan_v(x, y);
		}
	}
	return buf;
}

/* Output is Y | uv << 8, where uv is the U V byte pair read as a native
 * uint16_t. */
static uint32_t nv12_pixel(uint32_t x, uint32_t y)
{
	const uint8_t uv_bytes[2] = {chan_u(x / 2, y / 2), chan_v(x / 2, y / 2)};
	uint16_t uv;
	memcpy(&uv, uv_bytes, sizeof(uv));
	return chan_y(x, y) | (uint32_t)uv << 8;
}

static void test_decompress_nv12(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize = W * 4 + 8;
	const size_t out_size = (size_t)out_linesize * H;
	const uint32_t in_linesize[2] = {W, W + 4};
	uint8_t *planes[2] = {
		make_plane(chan_y, W, H, in_linesize[0]),
		make_uv_plane(W / 2, H / 2, in_linesize[1]),
	};
	const uint8_t *const input[2] = {planes[0], planes[1]};
	uint8_t *output = canary_buf(out_size);
	uint8_t *expected = canary_buf(out_size);

	for (uint32_t y = 0; y < H; y++)
		for (uint32_t x = 0; x < W; x++)
			put32(expected, (size_t)y * out_linesize + x * 4, nv12_pixel(x, y));

	decompress_nv12(input, in_linesize, 0, H, output, out_linesize);

	assert_buf_equal(output, expected, out_size);

	for (size_t i = 0; i < 2; i++)
		bfree(planes[i]);
	bfree(output);
	bfree(expected);
}

static void test_decompress_nv12_rows(void **state)
{
	UNUSED_PARAMETER(state);

	const uint32_t out_linesize = W * 4;
	const size_t out_size = (size_t)out_linesize * H;
	const uint32_t in_linesize[2] = {W, W};
	uint8_t *planes[2] = {
		make_plane(chan_y, W, H, in_linesize[0]),
		make_uv_plane(W / 2, H / 2, in_linesize[1]),
	};
	const uint8_t *const input[2] = {planes[0], planes[1]};
	uint8_t *output = canary_buf(out_size);
	uint8_t *expected = canary_buf(out_size);

	/* start_y 3, end_y 4: 1 / 2, so rows 2 and 3. */
	for (uint32_t y = 2; y < 4; y++)
		for (uint32_t x = 0; x < W; x++)
			put32(expected, (size_t)y * out_linesize + x * 4, nv12_pixel(x, y));

	decompress_nv12(input, in_linesize, 3, 4, output, out_linesize);

	assert_buf_equal(output, expected, out_size);

	for (size_t i = 0; i < 2; i++)
		bfree(planes[i]);
	bfree(output);
	bfree(expected);
}

/* decompress_422 copies each input dword d and follows it with a second dword
 * whose first (leading_lum) or second luma byte is replaced by the other one:
 * 0x44332211 gives 0x44332233 with leading_lum and 0x44334411 without. */
static uint32_t dup_luma(uint32_t dw, bool leading_lum)
{
	if (leading_lum)
		return (dw & 0xFFFFFF00) | ((dw >> 16) & 0xFF);
	return (dw & 0xFFFF00FF) | ((dw >> 16) & 0xFF00);
}

static void test_decompress_422_literals(void **state)
{
	UNUSED_PARAMETER(state);

	assert_int_equal(dup_luma(0x44332211, true), 0x44332233);
	assert_int_equal(dup_luma(0x44332211, false), 0x44334411);

	for (int leading = 0; leading <= 1; leading++) {
		uint8_t *input = canary_buf(8);
		uint8_t *output = canary_buf(16);
		put32(input, 0, 0x44332211);
		put32(input, 4, 0x88776655);

		/* in_linesize 4, out_linesize 8: min(4, 8) / 4 = 1 dword in, 2
		 * out; the second input dword is not read (see
		 * test_decompress_422_rows_in_bounds). */
		decompress_422(input, 4, 0, 1, output, 8, leading);

		uint32_t out[4];
		memcpy(out, output, sizeof(out));
		assert_int_equal(out[0], 0x44332211);
		assert_int_equal(out[1], leading ? 0x44332233 : 0x44334411);
		assert_int_equal(out[2], CANARY * 0x01010101u);
		assert_int_equal(out[3], CANARY * 0x01010101u);
		assert_int_equal(output[16], CANARY);

		bfree(input);
		bfree(output);
	}
}

/* Each row reads min(in_linesize, out_linesize) / 4 dwords, the
 * in_linesize bytes of the row, and writes twice as many dwords, its
 * out_linesize bytes (#3031; until then each row read and wrote twice that
 * and spilled into the next one). With in_linesize 8 and out_linesize 16, two
 * rows read 4 of the 6 dwords and write 8; the rest of the output keeps the
 * canary. */
static void test_decompress_422_rows_in_bounds(void **state)
{
	UNUSED_PARAMETER(state);

	static const uint32_t dwords[6] = {0x44332211, 0x88776655, 0xCCBBAA99, 0x00FFEEDD, 0x13579BDF, 0x2468ACE0};

	for (int leading = 0; leading <= 1; leading++) {
		uint8_t *input = canary_buf(sizeof(dwords));
		uint8_t *output = canary_buf(48);
		uint8_t *expected = canary_buf(48);
		memcpy(input, dwords, sizeof(dwords));

		for (size_t i = 0; i < 4; i++) {
			put32(expected, i * 8, dwords[i]);
			put32(expected, i * 8 + 4, dup_luma(dwords[i], leading));
		}

		decompress_422(input, 8, 0, 2, output, 16, leading);

		assert_buf_equal(output, expected, 48);

		bfree(input);
		bfree(output);
		bfree(expected);
	}
}

/* One packed 4:2:2 row of 4 pixels (in_linesize 8) converts to one row of 4
 * 32-bit pixels (out_linesize 16): it reads the row's 2 dwords, not the next
 * row's, writes 4, and leaves the canary after the output (#3031). */
static void test_decompress_422_one_row(void **state)
{
	UNUSED_PARAMETER(state);

	for (int leading = 0; leading <= 1; leading++) {
		uint8_t *input = canary_buf(16);
		uint8_t *output = canary_buf(16);
		uint8_t *expected = canary_buf(16);
		put32(input, 0, 0x44332211);
		put32(input, 4, 0x88776655);
		put32(input, 8, 0xCCBBAA99);
		put32(input, 12, 0x00FFEEDD);
		put32(expected, 0, 0x44332211);
		put32(expected, 4, leading ? 0x44332233 : 0x44334411);
		put32(expected, 8, 0x88776655);
		put32(expected, 12, leading ? 0x88776677 : 0x88778855);

		decompress_422(input, 8, 0, 1, output, 16, leading);

		assert_buf_equal(output, expected, 16);

		bfree(input);
		bfree(output);
		bfree(expected);
	}
}

/* start_y == end_y writes nothing. */
static void test_empty_row_range(void **state)
{
	UNUSED_PARAMETER(state);

	uint8_t *uyvx = make_uyvx(W, H, UYVX_LINESIZE);
	uint8_t *planes[3] = {make_plane(chan_y, W, H, W), make_uv_plane(W / 2, H / 2, W), make_plane(chan_v, W, H, W)};
	const uint8_t *const input[3] = {planes[0], planes[1], planes[2]};
	uint8_t *output[3] = {canary_buf(W * H * 4), canary_buf(W * H * 4), canary_buf(W * H * 4)};
	uint8_t *expected = canary_buf(W * H * 4);
	const uint32_t linesizes[3] = {W, W, W};

	compress_uyvx_to_i420(uyvx, UYVX_LINESIZE, 2, 2, output, linesizes);
	compress_uyvx_to_nv12(uyvx, UYVX_LINESIZE, 2, 2, output, linesizes);
	convert_uyvx_to_i444(uyvx, UYVX_LINESIZE, 2, 2, output, linesizes);
	decompress_420(input, linesizes, 2, 2, output[0], W * 4);
	decompress_nv12(input, linesizes, 2, 2, output[0], W * 4);
	decompress_422(planes[0], W, 2, 2, output[0], W * 4, true);
	decompress_422(planes[0], W, 2, 2, output[0], W * 4, false);

	for (size_t i = 0; i < 3; i++)
		assert_buf_equal(output[i], expected, W * H * 4);

	bfree(uyvx);
	bfree(expected);
	for (size_t i = 0; i < 3; i++) {
		bfree(planes[i]);
		bfree(output[i]);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_i420),
		cmocka_unit_test(test_i420_chroma_rounds_down),
		cmocka_unit_test(test_i420_rows),
		cmocka_unit_test(test_i420_out_linesize_limits_width),
		cmocka_unit_test(test_i420_v_plane_uses_u_linesize),
		cmocka_unit_test(test_nv12),
		cmocka_unit_test(test_nv12_rows_and_linesize),
		cmocka_unit_test(test_i444),
		cmocka_unit_test(test_i444_planes_use_luma_linesize),
		cmocka_unit_test(test_decompress_420),
		cmocka_unit_test(test_decompress_420_odd_rows_round_down),
		cmocka_unit_test(test_decompress_420_width_from_luma_linesize),
		cmocka_unit_test(test_decompress_nv12),
		cmocka_unit_test(test_decompress_nv12_rows),
		cmocka_unit_test(test_decompress_422_literals),
		cmocka_unit_test(test_decompress_422_rows_in_bounds),
		cmocka_unit_test(test_decompress_422_one_row),
		cmocka_unit_test(test_empty_row_range),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
