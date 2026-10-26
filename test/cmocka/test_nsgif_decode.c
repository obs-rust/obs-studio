/*
 * The vendored libnsgif (libobs/graphics/libnsgif), decoded directly: a
 * one-frame GIF goes to FFmpeg in gs_image_file, so test_image_file does not
 * reach libnsgif for it. Each case decodes a GIF and compares frame 0 with
 * the RGB values of the reference image next to it (plain PPM, P3), as
 * libnsgif's own test suite does.
 *
 * libnsgif-lzweod.gif and .ppm are libnsgif's test/data/lzweod.gif and .ppm
 * (netsurf-browser/libnsgif 9abf1190d, MIT, see
 * libobs/graphics/libnsgif/LICENSE.libnsgif): the last LZW code ends on a
 * byte boundary at the end of the data, which the code reader's slow path
 * lost before upstream 2255b2f74 (obs-rust/obs-studio#2465).
 *
 * The lzw_* cases characterize the LZW decoder (lzw.c) through lzw.h before
 * its port to obs-graphics (#2466). An emitter writes exact code streams
 * with the decoder's own width rule: the width grows when an entry is added
 * at table_size == code_max, up to LZW_CODE_MAX, and nothing is added once
 * the table has 4096 entries.
 *
 * The blocks_* cases characterize the GIF block parsers (gif-blocks.c)
 * through gif-blocks.h before their port to obs-graphics (#2488): the
 * header, the logical screen descriptor, the extensions before a frame,
 * the image descriptor and the colour tables, on hand-written bytes.
 *
 * The frame_* cases characterize the frame pixel decode and disposal
 * (gif-frame.c) through gif-frame.h before their port to obs-graphics
 * (#2501): frames at every position relative to the canvas, clipping,
 * interlacing, transparency, the two decode paths, the end of the data,
 * the LZW results, and the restore and recover steps.
 *
 * The gif_* cases characterize the state machine and the public API
 * (gif.c) through nsgif.h before its port to obs-graphics (#2510), on GIFs
 * that a small writer builds: the strings, the colour layouts, whole and
 * incremental scans, the screen size quirk, nsgif_data_complete, the frame
 * order and loops of nsgif_frame_prepare, the disposal across frames in
 * nsgif_frame_decode and the bitmap callbacks. The pixel values assume a
 * little-endian host (every CI host is).
 *
 * test_nsgif_decode_rust builds the same cases with NSGIF_TEST_RUST:
 * without libnsgif's C, against the Rust shims of obs-graphics (nsgif.h,
 * gif-blocks.h, gif-frame.h and lzw.h are the boundary).
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#ifdef _MSC_VER
#pragma warning(push)
#pragma warning(disable : 4244 4267)
#endif
#ifndef NSGIF_TEST_RUST
#include "graphics/libnsgif/lzw.c.h"
#include "graphics/libnsgif/gif-blocks.c.h"
#include "graphics/libnsgif/gif-frame.c.h"
#include "graphics/libnsgif/gif.c.h"
#else
#include <stdbool.h>
#include <stdint.h>
#include "graphics/libnsgif/nsgif.h"
#include "graphics/libnsgif/lzw.h"
#include "graphics/libnsgif/gif-blocks.h"
#include "graphics/libnsgif/gif-frame.h"
#endif
#ifdef _MSC_VER
#pragma warning(pop)
#endif

#define DATA(name) NSGIF_DATA_DIR "/" name

static nsgif_bitmap_t *bitmap_create(int width, int height)
{
	return calloc((size_t)width * (size_t)height, 4);
}

static void bitmap_destroy(nsgif_bitmap_t *bitmap)
{
	free(bitmap);
}

static uint8_t *bitmap_get_buffer(nsgif_bitmap_t *bitmap)
{
	return (uint8_t *)bitmap;
}

static const nsgif_bitmap_cb_vt bitmap_vt = {
	.create = bitmap_create,
	.destroy = bitmap_destroy,
	.get_buffer = bitmap_get_buffer,
};

static uint8_t *read_file(const char *path, size_t *size)
{
	FILE *f = fopen(path, "rb");
	assert_non_null(f);
	assert_int_equal(fseek(f, 0, SEEK_END), 0);
	long length = ftell(f);
	assert_true(length > 0);
	assert_int_equal(fseek(f, 0, SEEK_SET), 0);
	uint8_t *data = malloc((size_t)length);
	assert_non_null(data);
	assert_int_equal(fread(data, 1, (size_t)length, f), (size_t)length);
	fclose(f);
	*size = (size_t)length;
	return data;
}

/* The P3 reference: its width, height and the RGB values of frame 0. Lines
 * starting with '#' are comments. */
static unsigned *read_ppm(const char *path, unsigned *width, unsigned *height)
{
	size_t size;
	char *text = (char *)read_file(path, &size);
	char *copy = calloc(size + 1, 1);
	assert_non_null(copy);
	size_t n = 0;
	for (size_t i = 0; i < size; i++) {
		if (text[i] == '#') {
			while (i < size && text[i] != '\n')
				i++;
		}
		if (i < size)
			copy[n++] = text[i];
	}
	free(text);

	unsigned max;
	int used;
	assert_int_equal(sscanf(copy, "P3 %u %u %u%n", width, height, &max, &used), 3);
	/* libnsgif writes its references with a maximum of 256. */
	assert_true(max == 255 || max == 256);
	size_t count = (size_t)*width * *height * 3;
	unsigned *rgb = calloc(count, sizeof(*rgb));
	assert_non_null(rgb);
	const char *p = copy + used;
	for (size_t i = 0; i < count; i++) {
		int step;
		assert_int_equal(sscanf(p, "%u%n", &rgb[i], &step), 1);
		p += step;
	}
	free(copy);
	return rgb;
}

static void assert_frame0_matches(const char *gif_path, const char *ppm_path)
{
	size_t size;
	uint8_t *data = read_file(gif_path, &size);
	unsigned width, height;
	unsigned *expected = read_ppm(ppm_path, &width, &height);

	nsgif_t *gif;
	assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
	assert_int_equal(nsgif_data_scan(gif, size, data), NSGIF_OK);
	nsgif_data_complete(gif);
	const nsgif_info_t *info = nsgif_get_info(gif);
	assert_int_equal(info->width, width);
	assert_int_equal(info->height, height);

	nsgif_bitmap_t *bitmap = NULL;
	assert_int_equal(nsgif_frame_decode(gif, 0, &bitmap), NSGIF_OK);
	assert_non_null(bitmap);
	const uint8_t *pixels = bitmap_get_buffer(bitmap);
	for (size_t px = 0; px < (size_t)width * height; px++) {
		for (size_t c = 0; c < 3; c++) {
			if (pixels[4 * px + c] != expected[3 * px + c]) {
				fprintf(stderr, "pixel %u channel %u: %u, expected %u\n", (unsigned)px, (unsigned)c,
					pixels[4 * px + c], expected[3 * px + c]);
				/* fail_msg() does not link on Windows (ci/test_cmocka_windows_diagnostics.py). */
				fail();
			}
		}
	}

	nsgif_destroy(gif);
	free(expected);
	free(data);
}

/* The last code ends on a byte boundary at the end of the data: all three
 * pixels, the last one blue. */
static void lzw_code_on_byte_boundary_at_end_of_data(void **state)
{
	(void)state;
	assert_frame0_matches(DATA("libnsgif-lzweod.gif"), DATA("libnsgif-lzweod.ppm"));
}

/* ---------------------------------------------------------------------- */
/* The LZW decoder (lzw.c) through lzw.h. */

struct lzw_stream {
	uint8_t raw[1 << 16];
	size_t bits;
	unsigned min, width, table_size, clear, eoi;
	bool first;
};

static void lzw_put(struct lzw_stream *st, unsigned code, unsigned width)
{
	for (unsigned i = 0; i < width; i++, st->bits++) {
		if ((code >> i) & 1)
			st->raw[st->bits >> 3] |= (uint8_t)(1u << (st->bits & 7));
	}
}

static void lzw_begin(struct lzw_stream *st, unsigned min)
{
	memset(st, 0, sizeof(*st));
	st->min = min;
	st->clear = 1u << min;
	st->eoi = st->clear + 1;
	st->width = min + 1;
}

static void lzw_clear(struct lzw_stream *st)
{
	lzw_put(st, st->clear, st->width);
	st->width = st->min + 1;
	st->table_size = st->eoi + 1;
	st->first = true;
}

/* A data code: after the first code since a clear, each one adds an entry
 * while the table has room, and the width grows as the decoder's does. */
static void lzw_code(struct lzw_stream *st, unsigned code)
{
	lzw_put(st, code, st->width);
	if (st->first) {
		st->first = false;
	} else if (st->table_size < 4096) {
		if (st->table_size == (1u << st->width) - 1 && st->width < 12)
			st->width++;
		st->table_size++;
	}
}

static void lzw_eoi(struct lzw_stream *st)
{
	lzw_put(st, st->eoi, st->width);
}

/* The raw bits in sub-blocks of block_size bytes, then a zero-length
 * sub-block. Returns the length. */
static size_t lzw_pack(const struct lzw_stream *st, size_t block_size, uint8_t *out)
{
	size_t bytes = (st->bits + 7) / 8, n = 0;
	for (size_t pos = 0; pos < bytes; pos += block_size) {
		size_t len = bytes - pos < block_size ? bytes - pos : block_size;
		out[n++] = (uint8_t)len;
		memcpy(out + n, st->raw + pos, len);
		n += len;
	}
	out[n++] = 0;
	return n;
}

/* Decode with lzw_decode until a result other than LZW_OK; the values go
 * to out. Returns that result. */
static lzw_result lzw_decode_all(struct lzw_ctx *ctx, uint8_t *out, size_t cap, size_t *written)
{
	*written = 0;
	for (;;) {
		const uint8_t *data;
		uint32_t n;
		lzw_result res = lzw_decode(ctx, &data, &n);
		assert_true(*written + n <= cap);
		memcpy(out + *written, data, n);
		*written += n;
		if (res != LZW_OK)
			return res;
	}
}

static struct lzw_stream st;
static uint8_t packed[1 << 17];
static uint8_t out[1 << 17];

static void lzw_roots_at_every_code_size(void **state)
{
	(void)state;
	struct lzw_ctx *ctx;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);
	for (unsigned min = 2; min <= 8; min++) {
		lzw_begin(&st, min);
		lzw_clear(&st);
		for (unsigned v = 0; v < (1u << min); v++)
			lzw_code(&st, v);
		lzw_eoi(&st);
		size_t len = lzw_pack(&st, 255, packed), written;
		assert_int_equal(lzw_decode_init(ctx, (uint8_t)min, packed, len, 0), LZW_OK);
		assert_int_equal(lzw_decode_all(ctx, out, sizeof(out), &written), LZW_EOI_CODE);
		assert_int_equal(written, 1u << min);
		for (unsigned v = 0; v < (1u << min); v++)
			assert_int_equal(out[v], v);
	}
	lzw_context_destroy(ctx);
}

/* A code equal to table_size (KwKwK) is the previous record plus its own
 * first value. A clear resets the table and the width. */
static void lzw_kwkwk_and_clear(void **state)
{
	(void)state;
	struct lzw_ctx *ctx;
	size_t len, written;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);

	lzw_begin(&st, 2);
	lzw_clear(&st);
	lzw_code(&st, 1);
	lzw_code(&st, 6);
	lzw_eoi(&st);
	len = lzw_pack(&st, 255, packed);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, len, 0), LZW_OK);
	assert_int_equal(lzw_decode_all(ctx, out, sizeof(out), &written), LZW_EOI_CODE);
	assert_int_equal(written, 3);
	assert_memory_equal(out, ((uint8_t[]){1, 1, 1}), 3);

	lzw_begin(&st, 2);
	lzw_clear(&st);
	lzw_code(&st, 0);
	lzw_code(&st, 1);
	lzw_clear(&st);
	lzw_code(&st, 3);
	lzw_code(&st, 6);
	lzw_eoi(&st);
	len = lzw_pack(&st, 255, packed);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, len, 0), LZW_OK);
	assert_int_equal(lzw_decode_all(ctx, out, sizeof(out), &written), LZW_EOI_CODE);
	assert_int_equal(written, 5);
	assert_memory_equal(out, ((uint8_t[]){0, 1, 3, 3, 3}), 5);

	lzw_context_destroy(ctx);
}

static void lzw_bad_codes(void **state)
{
	(void)state;
	struct lzw_ctx *ctx;
	size_t len, written;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);

	/* A code above table_size: the values before it are written. */
	lzw_begin(&st, 2);
	lzw_clear(&st);
	lzw_code(&st, 0);
	lzw_put(&st, 7, st.width);
	len = lzw_pack(&st, 255, packed);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, len, 0), LZW_OK);
	assert_int_equal(lzw_decode_all(ctx, out, sizeof(out), &written), LZW_BAD_CODE);
	assert_int_equal(written, 1);
	assert_int_equal(out[0], 0);

	/* The first code after a clear must be a root: EOI, or a table code. */
	lzw_begin(&st, 2);
	lzw_clear(&st);
	lzw_eoi(&st);
	len = lzw_pack(&st, 255, packed);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, len, 0), LZW_BAD_ICODE);
	lzw_begin(&st, 2);
	lzw_clear(&st);
	lzw_put(&st, 6, st.width);
	len = lzw_pack(&st, 255, packed);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, len, 0), LZW_BAD_ICODE);

	/* A sequence of clears is one clear. */
	lzw_begin(&st, 2);
	lzw_clear(&st);
	lzw_clear(&st);
	lzw_clear(&st);
	lzw_code(&st, 2);
	lzw_eoi(&st);
	len = lzw_pack(&st, 255, packed);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, len, 0), LZW_OK);
	assert_int_equal(lzw_decode_all(ctx, out, sizeof(out), &written), LZW_EOI_CODE);
	assert_int_equal(written, 1);
	assert_int_equal(out[0], 2);

	/* Minimum code sizes: 11 works, 12 and above are refused. */
	lzw_begin(&st, 11);
	lzw_clear(&st);
	lzw_code(&st, 2047);
	lzw_eoi(&st);
	len = lzw_pack(&st, 255, packed);
	assert_int_equal(lzw_decode_init(ctx, 11, packed, len, 0), LZW_OK);
	assert_int_equal(lzw_decode_init(ctx, 12, packed, len, 0), LZW_BAD_ICODE);
	assert_int_equal(lzw_decode_init(ctx, 255, packed, len, 0), LZW_BAD_ICODE);

	lzw_context_destroy(ctx);
}

/* The end of the data: a zero-length sub-block is LZW_OK_EOD, missing data
 * is LZW_NO_DATA, both on init and while decoding. */
static void lzw_end_of_data(void **state)
{
	(void)state;
	struct lzw_ctx *ctx;
	size_t len, written;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);

	lzw_begin(&st, 2);
	lzw_clear(&st);
	lzw_code(&st, 0);
	lzw_code(&st, 1);
	len = lzw_pack(&st, 255, packed);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, len, 0), LZW_OK);
	/* 3 + 3 + 3 bits in 2 bytes: the 7 padding bits read as two more codes
	 * 0, then the zero-length sub-block ends the data. */
	assert_int_equal(lzw_decode_all(ctx, out, sizeof(out), &written), LZW_OK_EOD);
	assert_int_equal(written, 4);
	assert_memory_equal(out, ((uint8_t[]){0, 1, 0, 0}), 4);

	/* Without the terminating sub-block: the same values, then no data. */
	assert_int_equal(lzw_decode_init(ctx, 2, packed, len - 1, 0), LZW_OK);
	assert_int_equal(lzw_decode_all(ctx, out, sizeof(out), &written), LZW_NO_DATA);
	assert_int_equal(written, 4);
	assert_memory_equal(out, ((uint8_t[]){0, 1, 0, 0}), 4);

	/* A zero-length first sub-block, an empty buffer, a start past it. */
	packed[0] = 0;
	assert_int_equal(lzw_decode_init(ctx, 2, packed, 1, 0), LZW_OK_EOD);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, 0, 0), LZW_NO_DATA);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, 1, 5), LZW_NO_DATA);
	/* A sub-block that reaches the end of the data is refused: its size
	 * byte plus its length must stay below input_length. */
	packed[0] = 1;
	packed[1] = 0;
	assert_int_equal(lzw_decode_init(ctx, 2, packed, 1, 0), LZW_NO_DATA);

	lzw_context_destroy(ctx);
}

/* The same stream in sub-blocks of 1, 2, 3 and 255 bytes, so 9- to 12-bit
 * codes span up to three sub-blocks, decodes the same. */
static void lzw_codes_across_sub_blocks(void **state)
{
	(void)state;
	struct lzw_ctx *ctx;
	static uint8_t first[1 << 17];
	size_t first_written = 0;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);

	lzw_begin(&st, 8);
	lzw_clear(&st);
	for (unsigned i = 0; i < 4000; i++)
		lzw_code(&st, (i * 7) % 256);
	assert_int_equal(st.width, 12);
	lzw_eoi(&st);

	const size_t sizes[] = {255, 1, 2, 3};
	for (size_t s = 0; s < sizeof(sizes) / sizeof(sizes[0]); s++) {
		size_t len = lzw_pack(&st, sizes[s], packed), written;
		assert_int_equal(lzw_decode_init(ctx, 8, packed, len, 0), LZW_OK);
		assert_int_equal(lzw_decode_all(ctx, out, sizeof(out), &written), LZW_EOI_CODE);
		assert_int_equal(written, 4000);
		if (s == 0) {
			memcpy(first, out, written);
			first_written = written;
			for (unsigned i = 0; i < 4000; i++)
				assert_int_equal(out[i], (i * 7) % 256);
		} else {
			assert_int_equal(written, first_written);
			assert_memory_equal(out, first, written);
		}
	}
	lzw_context_destroy(ctx);
}

/* The last code (EOI) ending on a byte boundary at the end of the data, at
 * every width from 3 to 12 (#2465). */
static void lzw_eoi_on_byte_boundary(void **state)
{
	(void)state;
	struct lzw_ctx *ctx;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);
	for (unsigned min = 2; min <= 11; min++) {
		lzw_begin(&st, min);
		lzw_clear(&st);
		unsigned n = 0;
		while ((st.bits + st.width) % 8 != 0) {
			lzw_code(&st, 0);
			n++;
		}
		lzw_eoi(&st);
		size_t len = lzw_pack(&st, 255, packed), written;
		lzw_result res = lzw_decode_init(ctx, (uint8_t)min, packed, len, 0);
		if (n == 0) {
			/* EOI right after the clear is a bad initial code. */
			assert_int_equal(res, LZW_BAD_ICODE);
			continue;
		}
		assert_int_equal(res, LZW_OK);
		assert_int_equal(lzw_decode_all(ctx, out, sizeof(out), &written), LZW_EOI_CODE);
		assert_int_equal(written, n);
	}
	lzw_context_destroy(ctx);
}

/* A full table (4096 entries) adds nothing more; the width stays 12 and the
 * codes still decode. */
static void lzw_full_table(void **state)
{
	(void)state;
	struct lzw_ctx *ctx;
	size_t written;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);

	lzw_begin(&st, 8);
	lzw_clear(&st);
	for (unsigned i = 0; i < 3839; i++)
		lzw_code(&st, i % 256);
	assert_int_equal(st.table_size, 4096);
	for (unsigned i = 0; i < 100; i++)
		lzw_code(&st, 4095);
	lzw_eoi(&st);
	size_t len = lzw_pack(&st, 255, packed);
	assert_int_equal(lzw_decode_init(ctx, 8, packed, len, 0), LZW_OK);
	assert_int_equal(lzw_decode_all(ctx, out, sizeof(out), &written), LZW_EOI_CODE);
	/* Entry 4095 is the last one added, by the code 3838 % 256 after
	 * 3837 % 256: the record of 3837 % 256 plus 3838 % 256. */
	assert_int_equal(written, 3839 + 100 * 2);
	for (unsigned i = 0; i < 100; i++) {
		assert_int_equal(out[3839 + 2 * i], 3837 % 256);
		assert_int_equal(out[3839 + 2 * i + 1], 3838 % 256);
	}
	lzw_context_destroy(ctx);
}

/* lzw_decode fills its own 4096-value stack; a record that does not fit
 * continues on the next call. */
static void lzw_output_stack_full(void **state)
{
	(void)state;
	struct lzw_ctx *ctx;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);

	lzw_begin(&st, 2);
	lzw_clear(&st);
	lzw_code(&st, 3);
	size_t total = 1;
	for (unsigned k = 2; total + k <= 5000; k++) {
		lzw_code(&st, st.table_size);
		total += k;
	}
	lzw_eoi(&st);
	size_t len = lzw_pack(&st, 255, packed);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, len, 0), LZW_OK);

	const uint8_t *data;
	uint32_t n;
	assert_int_equal(lzw_decode(ctx, &data, &n), LZW_OK);
	assert_int_equal(n, 4096);
	size_t got = n;
	for (uint32_t i = 0; i < n; i++)
		assert_int_equal(data[i], 3);
	assert_int_equal(lzw_decode(ctx, &data, &n), LZW_EOI_CODE);
	got += n;
	for (uint32_t i = 0; i < n; i++)
		assert_int_equal(data[i], 3);
	assert_int_equal(got, total);
	lzw_context_destroy(ctx);
}

static const uint32_t colours[256] = {
	[0] = 0xff000000,
	[1] = 0xff0000ff,
	[2] = 0xff00ff00,
	[3] = 0xffff0000,
};

/* lzw_decode_map writes through the colour table, leaves the transparent
 * index's pixels as they were, and continues a record across calls. */
static void lzw_decode_map_colours_and_transparency(void **state)
{
	(void)state;
	struct lzw_ctx *ctx;
	uint32_t px[8];
	uint32_t written;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);

	lzw_begin(&st, 2);
	lzw_clear(&st);
	lzw_code(&st, 0);
	lzw_code(&st, 1);
	lzw_code(&st, 2);
	lzw_code(&st, 3);
	lzw_eoi(&st);
	size_t len = lzw_pack(&st, 255, packed);

	/* No transparency (256 and above). */
	const uint32_t no_transparency[] = {256, 0x102, 0xffffffff};
	for (size_t t = 0; t < 3; t++) {
		for (size_t i = 0; i < 8; i++)
			px[i] = 0xdeadbeef;
		assert_int_equal(lzw_decode_init_map(ctx, 2, no_transparency[t], colours, packed, len, 0), LZW_OK);
		assert_int_equal(lzw_decode_map(ctx, px, 4, &written), LZW_OK);
		assert_int_equal(written, 4);
		assert_memory_equal(px, colours, 4 * sizeof(uint32_t));
		assert_int_equal(px[4], 0xdeadbeef);
	}

	/* Index 2 transparent: its pixel keeps the previous frame's value. */
	for (size_t i = 0; i < 8; i++)
		px[i] = 0xdeadbeef;
	assert_int_equal(lzw_decode_init_map(ctx, 2, 2, colours, packed, len, 0), LZW_OK);
	assert_int_equal(lzw_decode_map(ctx, px, 8, &written), LZW_EOI_CODE);
	assert_int_equal(written, 4);
	assert_int_equal(px[0], colours[0]);
	assert_int_equal(px[1], colours[1]);
	assert_int_equal(px[2], 0xdeadbeef);
	assert_int_equal(px[3], colours[3]);

	/* An output of 1: one value per call. */
	assert_int_equal(lzw_decode_init_map(ctx, 2, 256, colours, packed, len, 0), LZW_OK);
	for (size_t i = 0; i < 4; i++) {
		assert_int_equal(lzw_decode_map(ctx, px, 1, &written), LZW_OK);
		assert_int_equal(written, 1);
		assert_int_equal(px[0], colours[i]);
	}
	assert_int_equal(lzw_decode_map(ctx, px, 1, &written), LZW_EOI_CODE);
	assert_int_equal(written, 0);

	/* An output of 0 writes nothing and reads nothing. */
	assert_int_equal(lzw_decode_init_map(ctx, 2, 256, colours, packed, len, 0), LZW_OK);
	assert_int_equal(lzw_decode_map(ctx, px, 0, &written), LZW_OK);
	assert_int_equal(written, 0);

	lzw_context_destroy(ctx);
}

static void lzw_decode_map_needs_a_colour_table(void **state)
{
	(void)state;
	struct lzw_ctx *ctx;
	uint32_t px[4];
	uint32_t written = 99;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);

	lzw_begin(&st, 2);
	lzw_clear(&st);
	lzw_code(&st, 0);
	lzw_eoi(&st);
	size_t len = lzw_pack(&st, 255, packed);

	assert_int_equal(lzw_decode_init_map(ctx, 2, 256, NULL, packed, len, 0), LZW_BAD_PARAM);
	assert_int_equal(lzw_decode_init(ctx, 2, packed, len, 0), LZW_OK);
	assert_int_equal(lzw_decode_map(ctx, px, 4, &written), LZW_NO_COLOUR);
	assert_int_equal(written, 0);

	/* Kept, characterized, not endorsed: a plain init that fails does not
	 * reach the line that clears the colour table, so the table of an
	 * earlier lzw_decode_init_map stays. */
	assert_int_equal(lzw_decode_init_map(ctx, 2, 256, colours, packed, len, 0), LZW_OK);
	assert_int_equal(lzw_decode_init(ctx, 12, packed, len, 0), LZW_BAD_ICODE);
	assert_int_not_equal(lzw_decode_map(ctx, px, 4, &written), LZW_NO_COLOUR);

	lzw_context_destroy(ctx);
}

/* ---------------------------------------------------------------------- */
/* The GIF block parsers (gif-blocks.c) through gif-blocks.h. */

static const struct nsgif_colour_layout layout_rgba = {.r = 0, .g = 1, .b = 2, .a = 3};
static const struct nsgif_colour_layout layout_bgra = {.b = 0, .g = 1, .r = 2, .a = 3};

/* Frame fields before an extension: values no extension writes. */
static struct nsgif_blocks_frame_ext blocks_ext_unset(void)
{
	return (struct nsgif_blocks_frame_ext){
		.delay = 77,
		.disposal = 9,
		.transparency = false,
		.transparency_index = 0xFFFFFFFFu,
		.redraw_required = false,
	};
}

static void assert_ext_unset(const struct nsgif_blocks_frame_ext *ext)
{
	assert_int_equal(ext->delay, 77);
	assert_int_equal(ext->disposal, 9);
	assert_false(ext->transparency);
	assert_int_equal(ext->transparency_index, 0xFFFFFFFFu);
	assert_false(ext->redraw_required);
}

/* "GIF" is required; the version only when strict. Six bytes from pos. */
static void blocks_header(void **state)
{
	(void)state;
	size_t pos;

	pos = 0;
	assert_int_equal(nsgif_blocks_header((const uint8_t *)"GIF89a", 6, &pos, true), NSGIF_OK);
	assert_int_equal(pos, 6);
	pos = 0;
	assert_int_equal(nsgif_blocks_header((const uint8_t *)"GIF87a", 6, &pos, true), NSGIF_OK);
	assert_int_equal(pos, 6);

	/* Kept, characterized, not endorsed: lenient (nsgif_data_scan) takes
	 * any version. */
	pos = 0;
	assert_int_equal(nsgif_blocks_header((const uint8_t *)"GIF90a", 6, &pos, true), NSGIF_ERR_DATA);
	assert_int_equal(pos, 0);
	assert_int_equal(nsgif_blocks_header((const uint8_t *)"GIF90a", 6, &pos, false), NSGIF_OK);
	assert_int_equal(pos, 6);

	pos = 0;
	assert_int_equal(nsgif_blocks_header((const uint8_t *)"GIX89a", 6, &pos, false), NSGIF_ERR_DATA);
	assert_int_equal(nsgif_blocks_header((const uint8_t *)"GIF89a", 5, &pos, false), NSGIF_ERR_END_OF_DATA);
	assert_int_equal(pos, 0);

	/* From a position inside the data. */
	pos = 3;
	assert_int_equal(nsgif_blocks_header((const uint8_t *)"xyzGIF89a", 9, &pos, true), NSGIF_OK);
	assert_int_equal(pos, 9);
	pos = 4;
	assert_int_equal(nsgif_blocks_header((const uint8_t *)"xyzGIF89a", 9, &pos, true), NSGIF_ERR_END_OF_DATA);
	assert_int_equal(pos, 4);
}

/* Seven bytes: width, height, the packed fields, background, aspect. */
static void blocks_screen(void **state)
{
	(void)state;
	const uint8_t lsd[] = {'G', 'I', 'F', '8', '9', 'a', 0x34, 0x12, 0x02, 0x01, 0xF5, 0x07, 0x31};
	struct nsgif_screen screen;
	size_t pos = 6;

	assert_int_equal(nsgif_blocks_screen(lsd, sizeof(lsd), &pos, &screen), NSGIF_OK);
	assert_int_equal(pos, 13);
	assert_int_equal(screen.width, 0x1234);
	assert_int_equal(screen.height, 0x0102);
	assert_true(screen.global_palette);
	assert_int_equal(screen.colour_table_size, 64); /* 2 << 5 */
	assert_int_equal(screen.bg_index, 7);
	assert_int_equal(screen.aspect_ratio, 0x31);
	assert_int_equal(screen.loop_max, 1);

	/* Without the flag, the size bits still give a size. */
	const uint8_t no_table[] = {0x10, 0x00, 0x08, 0x00, 0x07, 0xFF, 0x00};
	pos = 0;
	assert_int_equal(nsgif_blocks_screen(no_table, sizeof(no_table), &pos, &screen), NSGIF_OK);
	assert_false(screen.global_palette);
	assert_int_equal(screen.colour_table_size, 256);
	assert_int_equal(screen.bg_index, 255);

	/* Six bytes: nothing is set and pos stays. */
	memset(&screen, 0xAB, sizeof(screen));
	pos = 0;
	assert_int_equal(nsgif_blocks_screen(no_table, 6, &pos, &screen), NSGIF_ERR_END_OF_DATA);
	assert_int_equal(pos, 0);
	assert_int_equal(screen.width, 0xABABABABu);
}

/* The screen sizes that broken GIFs report become 1x1. */
static void blocks_screen_size_quirk(void **state)
{
	(void)state;
	static const struct {
		uint32_t w, h, out_w, out_h;
	} cases[] = {
		{640, 480, 1, 1},     {640, 512, 1, 1},     {800, 600, 1, 1},
		{1024, 768, 1, 1},    {1280, 1024, 1, 1},   {1600, 1200, 1, 1},
		{0, 5, 1, 1},         {5, 0, 1, 1},         {2049, 5, 1, 1},
		{5, 2049, 1, 1},      {65535, 65535, 1, 1}, {2048, 2048, 2048, 2048},
		{480, 640, 480, 640}, {640, 481, 640, 481}, {1920, 1080, 1920, 1080},
		{1, 1, 1, 1},
	};
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		uint32_t w = cases[i].w, h = cases[i].h;
		nsgif_blocks_screen_size_quirk(&w, &h);
		assert_int_equal(w, cases[i].out_w);
		assert_int_equal(h, cases[i].out_h);
	}
}

/* entries r,g,b triplets into the layout, alpha 0xff; every table size. */
static void blocks_colour_table(void **state)
{
	(void)state;
	uint8_t rgb[3 * 256];
	uint32_t table[NSGIF_MAX_COLOURS];
	size_t used;

	for (size_t i = 0; i < sizeof(rgb); i++)
		rgb[i] = (uint8_t)(i * 7 + 1);

	for (unsigned bits = 0; bits < 8; bits++) {
		size_t entries = 2u << bits;
		memset(table, 0x5A, sizeof(table));
		used = 0;
		assert_int_equal(nsgif_blocks_colour_table(table, &layout_rgba, entries, rgb, entries * 3, &used, true),
				 NSGIF_OK);
		assert_int_equal(used, entries * 3);
		for (size_t e = 0; e < entries; e++) {
			const uint8_t *px = (const uint8_t *)&table[e];
			assert_int_equal(px[0], rgb[3 * e]);
			assert_int_equal(px[1], rgb[3 * e + 1]);
			assert_int_equal(px[2], rgb[3 * e + 2]);
			assert_int_equal(px[3], 0xFF);
		}
		/* Entries past the table stay. */
		if (entries < NSGIF_MAX_COLOURS)
			assert_int_equal(table[entries], 0x5A5A5A5Au);

		/* One byte short. */
		used = 99;
		assert_int_equal(nsgif_blocks_colour_table(table, &layout_rgba, entries, rgb, entries * 3 - 1, &used,
							   true),
				 NSGIF_ERR_END_OF_DATA);
		assert_int_equal(used, 99);
	}

	/* Another layout. */
	assert_int_equal(nsgif_blocks_colour_table(table, &layout_bgra, 2, rgb, 6, &used, true), NSGIF_OK);
	const uint8_t *px = (const uint8_t *)&table[1];
	assert_int_equal(px[0], rgb[5]);
	assert_int_equal(px[1], rgb[4]);
	assert_int_equal(px[2], rgb[3]);
	assert_int_equal(px[3], 0xFF);

	/* Without decode: the length is checked and counted, the table stays. */
	memset(table, 0x5A, sizeof(table));
	assert_int_equal(nsgif_blocks_colour_table(table, &layout_rgba, 4, rgb, 12, &used, false), NSGIF_OK);
	assert_int_equal(used, 12);
	assert_int_equal(table[0], 0x5A5A5A5Au);
	assert_int_equal(nsgif_blocks_colour_table(table, &layout_rgba, 4, rgb, 11, &used, false),
			 NSGIF_ERR_END_OF_DATA);

	/* The plain decode (nsgif_local_palette) checks no length. */
	nsgif_blocks_colour_table_decode(table, &layout_rgba, 3, rgb);
	px = (const uint8_t *)&table[2];
	assert_int_equal(px[0], rgb[6]);
	assert_int_equal(px[3], 0xFF);
	assert_int_equal(table[3], 0x5A5A5A5Au);
}

/* The graphic control extension: delay, disposal (4 is 3) and the
 * transparent index; redraw for the two restore disposals. */
static void blocks_graphic_control(void **state)
{
	(void)state;
	for (unsigned disposal = 0; disposal < 8; disposal++) {
		/* introducer, label, size, packed, delay (2), index, terminator,
		 * then the next block. */
		uint8_t gce[] = {0x21, 0xF9, 0x04, (uint8_t)(disposal << 2 | 0xE0), 0x34, 0x12, 0x05, 0x00, 0x2C};
		struct nsgif_blocks_frame_ext ext = blocks_ext_unset();
		int loop_max = 1;
		size_t pos = 0;

		assert_int_equal(nsgif_blocks_extensions(gce, sizeof(gce), &pos, true, &ext, &loop_max), NSGIF_OK);
		assert_int_equal(pos, 8);
		assert_int_equal(ext.delay, 0x1234);
		unsigned expect = disposal == NSGIF_DISPOSAL_RESTORE_QUIRK ? NSGIF_DISPOSAL_RESTORE_PREV : disposal;
		assert_int_equal(ext.disposal, expect);
		assert_int_equal(ext.redraw_required,
				 expect == NSGIF_DISPOSAL_RESTORE_BG || expect == NSGIF_DISPOSAL_RESTORE_PREV);
		/* No transparency flag: transparency and its index stay. */
		assert_false(ext.transparency);
		assert_int_equal(ext.transparency_index, 0xFFFFFFFFu);
		assert_int_equal(loop_max, 1);
	}

	/* With the transparency flag. A second extension changes the delay
	 * and disposal; transparency stays set once set. */
	const uint8_t two[] = {0x21, 0xF9, 0x04, 0x09, 0x00, 0x00, 0x07, 0x00,
			       0x21, 0xF9, 0x04, 0x00, 0x02, 0x00, 0x09, 0x00};
	struct nsgif_blocks_frame_ext ext = blocks_ext_unset();
	int loop_max = 1;
	size_t pos = 0;
	assert_int_equal(nsgif_blocks_extensions(two, sizeof(two), &pos, true, &ext, &loop_max), NSGIF_OK);
	assert_int_equal(pos, sizeof(two));
	assert_int_equal(ext.delay, 2);
	assert_int_equal(ext.disposal, NSGIF_DISPOSAL_UNSPECIFIED);
	assert_false(ext.redraw_required);
	assert_true(ext.transparency);
	assert_int_equal(ext.transparency_index, 7);

	/* Without decode, the extensions are skipped and nothing is set. */
	ext = blocks_ext_unset();
	pos = 0;
	assert_int_equal(nsgif_blocks_extensions(two, sizeof(two), &pos, false, &ext, &loop_max), NSGIF_OK);
	assert_int_equal(pos, sizeof(two));
	assert_ext_unset(&ext);

	/* Kept, characterized, not endorsed: the size byte is not checked.
	 * Size 0 still reads four bytes of fields, and then the skip starts
	 * at the packed byte (5) as a sub-block size: it runs past the end
	 * and the fields are set although the result is an error. */
	const uint8_t size0[] = {0x21, 0xF9, 0x00, 0x05, 0x0A, 0x00, 0x07, 0x00};
	ext = blocks_ext_unset();
	pos = 0;
	assert_int_equal(nsgif_blocks_extensions(size0, sizeof(size0), &pos, true, &ext, &loop_max),
			 NSGIF_ERR_END_OF_DATA);
	assert_int_equal(pos, 0);
	assert_int_equal(ext.delay, 10);
	assert_int_equal(ext.disposal, NSGIF_DISPOSAL_NONE);
	assert_true(ext.transparency);
	assert_int_equal(ext.transparency_index, 7);
}

/* Fewer than six bytes from the label: the extension is short. */
static void blocks_graphic_control_short(void **state)
{
	(void)state;
	const uint8_t gce[] = {0x21, 0xF9, 0x04, 0x05, 0x0A};
	struct nsgif_blocks_frame_ext ext = blocks_ext_unset();
	int loop_max = 1;
	size_t pos = 0;

	for (size_t len = 1; len <= sizeof(gce); len++) {
		pos = 0;
		assert_int_equal(nsgif_blocks_extensions(gce, len, &pos, true, &ext, &loop_max), NSGIF_ERR_END_OF_DATA);
		assert_int_equal(pos, 0);
		assert_ext_unset(&ext);
	}
	/* Without decode, the fields are not read and the skip needs only the
	 * size byte. Kept, characterized, not endorsed: once that is there, a
	 * skip past the end is not an error and pos stops at the end. */
	for (size_t len = 1; len <= sizeof(gce); len++) {
		pos = 0;
		assert_int_equal(nsgif_blocks_extensions(gce, len, &pos, false, &ext, &loop_max),
				 len < 3 ? NSGIF_ERR_END_OF_DATA : NSGIF_OK);
		assert_int_equal(pos, len < 3 ? 0 : len);
	}

	/* A short extension after a complete one: the first one's fields are
	 * set, the result is an error and pos stays. */
	const uint8_t later[] = {0x21, 0xF9, 0x04, 0x01, 0x03, 0x00, 0x02, 0x00, 0x21, 0xF9, 0x04};
	pos = 0;
	assert_int_equal(nsgif_blocks_extensions(later, sizeof(later), &pos, true, &ext, &loop_max),
			 NSGIF_ERR_END_OF_DATA);
	assert_int_equal(pos, 0);
	assert_int_equal(ext.delay, 3);
	assert_true(ext.transparency);
	assert_int_equal(ext.transparency_index, 2);
}

/* NETSCAPE2.0 and ANIMEXTS1.0 with sub-block 3, id 1: the loop count. */
static void blocks_loop_count(void **state)
{
	(void)state;
	static const char *const idents[] = {"NETSCAPE2.0", "ANIMEXTS1.0", "NETSCAPE2.1", "XMP DataXMP"};
	static const struct {
		uint16_t value;
		int loop_max;
	} counts[] = {{0, 0}, {1, 2}, {5, 6}, {0xFFFF, 0x10000}};

	for (size_t i = 0; i < 4; i++) {
		for (size_t c = 0; c < 4; c++) {
			uint8_t app[19] = {0x21, 0xFF, 0x0B};
			memcpy(app + 3, idents[i], 11);
			app[14] = 0x03;
			app[15] = 0x01;
			app[16] = (uint8_t)(counts[c].value & 0xFF);
			app[17] = (uint8_t)(counts[c].value >> 8);
			app[18] = 0x00;
			struct nsgif_blocks_frame_ext ext = blocks_ext_unset();
			int loop_max = 1;
			size_t pos = 0;

			assert_int_equal(nsgif_blocks_extensions(app, sizeof(app), &pos, true, &ext, &loop_max),
					 NSGIF_OK);
			assert_int_equal(pos, sizeof(app));
			assert_int_equal(loop_max, i < 2 ? counts[c].loop_max : 1);
			assert_ext_unset(&ext);

			/* Without decode, the count stays. */
			loop_max = 1;
			pos = 0;
			assert_int_equal(nsgif_blocks_extensions(app, sizeof(app), &pos, false, &ext, &loop_max),
					 NSGIF_OK);
			assert_int_equal(loop_max, 1);
		}
	}

	/* A block size other than 11, a sub-block size other than 3 or an id
	 * other than 1: no loop count. */
	uint8_t app[19] = {0x21, 0xFF, 0x0B, 'N', 'E',  'T',  'S',  'C',  'A', 'P',
			   'E',  '2',  '.',  '0', 0x03, 0x01, 0x05, 0x00, 0x00};
	int loop_max = 1;
	size_t pos = 0;
	struct nsgif_blocks_frame_ext ext = blocks_ext_unset();
	app[15] = 0x02;
	assert_int_equal(nsgif_blocks_extensions(app, sizeof(app), &pos, true, &ext, &loop_max), NSGIF_OK);
	assert_int_equal(loop_max, 1);
	app[15] = 0x01;
	app[14] = 0x04;
	app[18] = 0x00;
	pos = 0;
	/* The sub-block of 4 now covers the terminator: the skip runs off. */
	assert_int_equal(nsgif_blocks_extensions(app, sizeof(app), &pos, true, &ext, &loop_max), NSGIF_ERR_END_OF_DATA);
	assert_int_equal(loop_max, 1);

	/* Fewer than 17 bytes from the label: short. */
	pos = 0;
	assert_int_equal(nsgif_blocks_extensions(app, 17, &pos, true, &ext, &loop_max), NSGIF_ERR_END_OF_DATA);
	assert_int_equal(pos, 0);
}

/* Comments, plain text and unknown labels are skipped by their
 * sub-blocks; the loop ends at the first byte that is not 0x21. */
static void blocks_skip_extensions(void **state)
{
	(void)state;
	const uint8_t data[] = {
		0x21, 0xFE, 0x03, 'a',  'b',  'c',  0x02, 'd',  'e', 0x00, /* comment: 0..9 */
		0x21, 0x01, 0x0C, 1,    2,    3,    4,    5,    6,   7,
		8,    9,    10,   11,   12,   0x01, 'x',  0x00, /* plain text: 10..27 */
		0x21, 0x99, 0x02, 0xAA, 0xBB, 0x00,             /* unknown: 28..33 */
		0x21, 0xFE, 0x00,                               /* empty comment: 34..36 */
		0x2C, 0x00,
	};
	struct nsgif_blocks_frame_ext ext = blocks_ext_unset();
	int loop_max = 1;
	size_t pos = 0;

	assert_int_equal(nsgif_blocks_extensions(data, sizeof(data), &pos, true, &ext, &loop_max), NSGIF_OK);
	assert_int_equal(pos, 37);
	assert_ext_unset(&ext);

	/* No extension: pos stays. Empty data too. */
	pos = 37;
	assert_int_equal(nsgif_blocks_extensions(data, sizeof(data), &pos, true, &ext, &loop_max), NSGIF_OK);
	assert_int_equal(pos, 37);
	pos = 0;
	assert_int_equal(nsgif_blocks_extensions(data, 0, &pos, true, &ext, &loop_max), NSGIF_OK);
	assert_int_equal(pos, 0);

	/* An introducer at the end of the data. */
	pos = 0;
	assert_int_equal(nsgif_blocks_extensions(data, 1, &pos, true, &ext, &loop_max), NSGIF_ERR_END_OF_DATA);
	assert_int_equal(pos, 0);

	/* A sub-block that runs past the end. */
	const uint8_t runs_off[] = {0x21, 0xFE, 0x05, 'a', 'b'};
	pos = 0;
	assert_int_equal(nsgif_blocks_extensions(runs_off, sizeof(runs_off), &pos, true, &ext, &loop_max),
			 NSGIF_ERR_END_OF_DATA);
	assert_int_equal(pos, 0);

	/* Kept, characterized, not endorsed: an extension size that skips past
	 * the end is not an error. pos stops at the end of the data. */
	const uint8_t size_past_end[] = {0x21, 0x99, 0xFF};
	pos = 0;
	assert_int_equal(nsgif_blocks_extensions(size_past_end, sizeof(size_past_end), &pos, true, &ext, &loop_max),
			 NSGIF_OK);
	assert_int_equal(pos, 3);
	const uint8_t size_to_end[] = {0x21, 0x99, 0x01, 0x00};
	pos = 0;
	assert_int_equal(nsgif_blocks_extensions(size_to_end, sizeof(size_to_end), &pos, true, &ext, &loop_max),
			 NSGIF_OK);
	assert_int_equal(pos, 4);
}

/* Ten bytes: separator, x, y, width, height, flags. */
static void blocks_image_descriptor(void **state)
{
	(void)state;
	const uint8_t desc[] = {0x00, 0x2C, 0x05, 0x00, 0x00, 0x00, 0x14, 0x00, 0x05, 0x00, 0xC3, 0x00};
	struct nsgif_blocks_image image;
	uint32_t width = 10, height = 10;
	size_t pos = 1;

	assert_int_equal(nsgif_blocks_image_descriptor(desc, sizeof(desc), &pos, true, true, &image, &width, &height),
			 NSGIF_OK);
	assert_int_equal(pos, 11);
	assert_int_equal(image.flags, 0xC3);
	assert_true(image.interlaced);
	assert_int_equal(image.rect.x0, 5);
	assert_int_equal(image.rect.y0, 0);
	assert_int_equal(image.rect.x1, 25);
	assert_int_equal(image.rect.y1, 5);
	/* The first frame grows the image, never shrinks it. */
	assert_int_equal(width, 25);
	assert_int_equal(height, 10);

	/* A later frame does not grow it. */
	width = 10;
	pos = 1;
	assert_int_equal(nsgif_blocks_image_descriptor(desc, sizeof(desc), &pos, true, false, &image, &width, &height),
			 NSGIF_OK);
	assert_int_equal(width, 10);

	/* 16-bit fields: x + w does not wrap. */
	const uint8_t far[] = {0x2C, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00};
	pos = 0;
	assert_int_equal(nsgif_blocks_image_descriptor(far, sizeof(far), &pos, true, true, &image, &width, &height),
			 NSGIF_OK);
	assert_int_equal(image.rect.x1, 131070);
	assert_int_equal(image.rect.y1, 131070);
	assert_false(image.interlaced);
	assert_int_equal(width, 131070);
	assert_int_equal(height, 131070);

	/* A bad separator with decode; nine bytes. */
	const uint8_t bad[] = {0x2D, 0, 0, 0, 0, 1, 0, 1, 0, 0};
	struct nsgif_blocks_image untouched;
	memset(&untouched, 0x5A, sizeof(untouched));
	image = untouched;
	pos = 0;
	assert_int_equal(nsgif_blocks_image_descriptor(bad, sizeof(bad), &pos, true, true, &image, &width, &height),
			 NSGIF_ERR_DATA_FRAME);
	assert_int_equal(pos, 0);
	assert_memory_equal(&image, &untouched, sizeof(image));
	pos = 0;
	assert_int_equal(nsgif_blocks_image_descriptor(bad, 9, &pos, true, true, &image, &width, &height),
			 NSGIF_ERR_END_OF_DATA);

	/* Kept, characterized, not endorsed: without decode (the frame decode
	 * pass) the separator is not checked and nothing is read. */
	width = 3;
	pos = 0;
	assert_int_equal(nsgif_blocks_image_descriptor(bad, sizeof(bad), &pos, false, true, &image, &width, &height),
			 NSGIF_OK);
	assert_int_equal(pos, 10);
	assert_int_equal(width, 3);
	assert_memory_equal(&image, &untouched, sizeof(image));
}

/* ---------------------------------------------------------------------- */
/* The frame pixel decode and disposal (gif-frame.c) through gif-frame.h. */

#define CANVAS_PX 4096
#define SENTINEL 0x5A5A5A5Au

static uint32_t canvas_px[CANVAS_PX];
static uint8_t frame_buf[1 << 14];

/* Palette entry i is 0xAB0000ii, so a pixel names its index. */
static uint32_t frame_palette[NSGIF_MAX_COLOURS];

static void frame_setup(void)
{
	for (uint32_t i = 0; i < NSGIF_MAX_COLOURS; i++)
		frame_palette[i] = 0xAB000000u | i;
	for (size_t i = 0; i < CANVAS_PX; i++)
		canvas_px[i] = SENTINEL;
}

/* A frame's image data at position 0 of frame_buf: minimum code size 8,
 * then n indices as root codes after a clear, an EOI when eoi, in
 * sub-blocks of block bytes with the zero-length terminator. */
static size_t frame_stream(const uint8_t *idx, size_t n, bool eoi, size_t block)
{
	lzw_begin(&st, 8);
	lzw_clear(&st);
	for (size_t i = 0; i < n; i++)
		lzw_code(&st, idx[i]);
	if (eoi)
		lzw_eoi(&st);
	frame_buf[0] = 8;
	return 1 + lzw_pack(&st, block, frame_buf + 1);
}

static struct nsgif_frame_area frame_area(uint32_t x, uint32_t y, uint32_t w, uint32_t h)
{
	return (struct nsgif_frame_area){
		.rect = {.x0 = x, .y0 = y, .x1 = x + w, .y1 = y + h},
		.interlaced = false,
		.display = true,
		.transparency = false,
		.transparency_index = NSGIF_MAX_COLOURS, /* none */
	};
}

static nsgif_error frame_decode(const struct nsgif_canvas *canvas, const struct nsgif_frame_area *area, size_t len,
				bool data_complete)
{
	struct lzw_ctx *ctx;
	assert_int_equal(lzw_context_create(&ctx), LZW_OK);
	nsgif_error ret = nsgif_frame_pixels_decode(ctx, frame_buf, len, 0, canvas, area, frame_palette, data_complete);
	lzw_context_destroy(ctx);
	return ret;
}

/* The pixel at (x, y) of a canvas with this row span. */
static uint32_t px_at(uint32_t rowspan, uint32_t x, uint32_t y)
{
	return canvas_px[(size_t)y * rowspan + x];
}

/* A full-width frame at x 0, not interlaced, on a canvas whose row span is
 * its width: the simple path (lzw_decode_map), row after row. */
static void frame_simple_full_frame(void **state)
{
	(void)state;
	uint8_t idx[12];
	for (int i = 0; i < 12; i++)
		idx[i] = (uint8_t)(i + 1);
	frame_setup();
	size_t len = frame_stream(idx, 12, true, 255);
	struct nsgif_canvas canvas = {.bitmap = canvas_px, .width = 4, .height = 3, .rowspan = 4, .background = 0};
	struct nsgif_frame_area area = frame_area(0, 0, 4, 3);

	assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_OK);
	for (uint32_t i = 0; i < 12; i++)
		assert_int_equal(canvas_px[i], 0xAB000000u | (i + 1));
	assert_int_equal(canvas_px[12], SENTINEL);

	/* A full-width frame lower down: rows from offset_y, clipped at the
	 * bottom. */
	frame_setup();
	area = frame_area(0, 1, 4, 3);
	assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_OK);
	assert_int_equal(canvas_px[3], SENTINEL);
	for (uint32_t i = 0; i < 8; i++)
		assert_int_equal(canvas_px[4 + i], 0xAB000000u | (i + 1));
	assert_int_equal(canvas_px[12], SENTINEL);
}

/* Any other frame: the complex path (lzw_decode). The rectangle is clipped
 * to the image on the right and at the bottom; rows step by the row span. */
static void frame_complex_offset_and_clip(void **state)
{
	(void)state;
	uint8_t idx[12];
	for (int i = 0; i < 12; i++)
		idx[i] = (uint8_t)(i + 1);
	frame_setup();
	size_t len = frame_stream(idx, 12, true, 255);
	/* Image 5x4 in a bitmap of 7 pixels per row; frame 4x3 at (3, 2). */
	struct nsgif_canvas canvas = {.bitmap = canvas_px, .width = 5, .height = 4, .rowspan = 7, .background = 0};
	struct nsgif_frame_area area = frame_area(3, 2, 4, 3);

	assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_OK);
	/* Rows 0 and 1 of the frame, columns 0 and 1: indices 1, 2 and 5, 6. */
	assert_int_equal(px_at(7, 3, 2), 0xAB000001u);
	assert_int_equal(px_at(7, 4, 2), 0xAB000002u);
	assert_int_equal(px_at(7, 3, 3), 0xAB000005u);
	assert_int_equal(px_at(7, 4, 3), 0xAB000006u);
	/* Nothing past the image: the clipped columns and the row span. */
	assert_int_equal(px_at(7, 5, 2), SENTINEL);
	assert_int_equal(px_at(7, 2, 2), SENTINEL);
	assert_int_equal(px_at(7, 3, 1), SENTINEL);
	assert_int_equal(px_at(7, 3, 4), SENTINEL);

	/* A full-width frame at x 0 in a bitmap wider than the image: the
	 * complex path, rows stepped by the row span. */
	frame_setup();
	canvas = (struct nsgif_canvas){.bitmap = canvas_px, .width = 4, .height = 3, .rowspan = 6};
	area = frame_area(0, 0, 4, 3);
	assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_OK);
	for (uint32_t r = 0; r < 3; r++) {
		for (uint32_t c = 0; c < 4; c++)
			assert_int_equal(px_at(6, c, r), 0xAB000000u | (r * 4 + c + 1));
		assert_int_equal(px_at(6, 4, r), SENTINEL);
	}

	/* The same pixels on both paths: a full frame with the row span equal
	 * to the width goes simple; interlaced off but x 1 goes complex. */
	frame_setup();
	canvas = (struct nsgif_canvas){.bitmap = canvas_px, .width = 4, .height = 3, .rowspan = 4};
	area = frame_area(0, 0, 4, 3);
	area.interlaced = true; /* forces the complex path */
	len = frame_stream(idx, 12, true, 255);
	assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_OK);
	uint32_t interlaced_rows[3] = {canvas_px[0], canvas_px[4], canvas_px[8]};
	/* Interlaced height 3: rows 0, 2, 1 in that order. */
	assert_int_equal(interlaced_rows[0], 0xAB000001u);
	assert_int_equal(interlaced_rows[1], 0xAB000009u);
	assert_int_equal(interlaced_rows[2], 0xAB000005u);
}

/* The row of the i-th decoded row of an interlaced frame (GIF89a, appendix
 * E): rows 0, 8, 16, ..., then 4, 12, ..., then 2, 6, ..., then 1, 3, .... */
static uint32_t interlaced_row(uint32_t height, uint32_t i)
{
	static const uint32_t start[4] = {0, 4, 2, 1}, step[4] = {8, 8, 4, 2};
	for (int pass = 0; pass < 4; pass++) {
		for (uint32_t y = start[pass]; y < height; y += step[pass]) {
			if (i-- == 0)
				return y;
		}
	}
	return UINT32_MAX;
}

static void frame_interlaced_heights(void **state)
{
	(void)state;
	static const uint32_t heights[] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 15, 16, 17, 31, 64};
	uint8_t idx[64];
	for (int i = 0; i < 64; i++)
		idx[i] = (uint8_t)i;

	for (size_t k = 0; k < sizeof(heights) / sizeof(heights[0]); k++) {
		uint32_t h = heights[k];
		frame_setup();
		size_t len = frame_stream(idx, h, true, 255);
		struct nsgif_canvas canvas = {.bitmap = canvas_px, .width = 1, .height = h, .rowspan = 1};
		struct nsgif_frame_area area = frame_area(0, 0, 1, h);
		area.interlaced = true;
		assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_OK);
		for (uint32_t i = 0; i < h; i++)
			assert_int_equal(canvas_px[interlaced_row(h, i)], 0xAB000000u | i);
		assert_int_equal(canvas_px[h], SENTINEL);
	}
}

/* The transparent index leaves its pixels; above 0xFF there is none. */
static void frame_transparency(void **state)
{
	(void)state;
	const uint8_t idx[6] = {1, 7, 2, 7, 7, 3};
	struct nsgif_canvas canvas = {.bitmap = canvas_px, .width = 3, .height = 2, .rowspan = 3};

	for (int complex = 0; complex < 2; complex++) {
		frame_setup();
		size_t len = frame_stream(idx, 6, true, 255);
		struct nsgif_frame_area area = frame_area(0, 0, 3, 2);
		area.interlaced = complex;
		area.transparency_index = 7;
		assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_OK);
		const uint32_t expect[6] = {0xAB000001u, SENTINEL, 0xAB000002u, SENTINEL, SENTINEL, 0xAB000003u};
		/* Interlaced height 2: rows 0 and 1 in order. */
		for (int i = 0; i < 6; i++)
			assert_int_equal(canvas_px[i], expect[i]);

		frame_setup();
		area.transparency_index = 0x100;
		assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_OK);
		assert_int_equal(canvas_px[1], 0xAB000007u);
	}
}

/* A frame that starts past the image, or is clipped to nothing, writes
 * nothing and is a success, without reading the data. */
static void frame_outside_the_image(void **state)
{
	(void)state;
	struct nsgif_canvas canvas = {.bitmap = canvas_px, .width = 4, .height = 3, .rowspan = 4};
	static const uint32_t rects[][4] = {
		{4, 0, 2, 2}, {0, 3, 2, 2}, {9, 9, 1, 1}, {1, 1, 0, 2}, {1, 1, 2, 0}, {0, 3, 4, 1}, {0, 0, 4, 0},
	};
	/* Data a decode would refuse: minimum code size 12. */
	frame_buf[0] = 12;
	frame_buf[1] = 0;

	for (size_t k = 0; k < sizeof(rects) / sizeof(rects[0]); k++) {
		frame_setup();
		struct nsgif_frame_area area = frame_area(rects[k][0], rects[k][1], rects[k][2], rects[k][3]);
		assert_int_equal(frame_decode(&canvas, &area, 2, false), NSGIF_OK);
		for (size_t i = 0; i < 12; i++)
			assert_int_equal(canvas_px[i], SENTINEL);
	}

	/* Inside the image, the same data is a bad frame on both paths. */
	for (int complex = 0; complex < 2; complex++) {
		frame_setup();
		struct nsgif_frame_area area = frame_area(0, 0, 4, 3);
		area.interlaced = complex;
		assert_int_equal(frame_decode(&canvas, &area, 2, false), NSGIF_ERR_DATA_FRAME);
		assert_int_equal(canvas_px[0], SENTINEL);
	}
}

/* Data that ends before the frame is full. */
static void frame_end_of_data(void **state)
{
	(void)state;
	const uint8_t idx[5] = {1, 2, 3, 4, 5};
	struct nsgif_canvas canvas = {.bitmap = canvas_px, .width = 4, .height = 2, .rowspan = 4};

	for (int complex = 0; complex < 2; complex++) {
		struct nsgif_frame_area area = frame_area(0, 0, 4, 2);
		area.interlaced = complex;

		/* An EOI after five pixels: a success, the rest left alone. */
		frame_setup();
		size_t len = frame_stream(idx, 5, true, 255);
		assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_OK);
		for (int i = 0; i < 5; i++)
			assert_int_equal(canvas_px[complex ? (i < 4 ? i : 4 + i - 4) : i],
					 0xAB000000u | (uint32_t)(i + 1));
		assert_int_equal(canvas_px[5], SENTINEL);

		/* The zero-length sub-block after five pixels, no EOI. */
		frame_setup();
		len = frame_stream(idx, 5, false, 255);
		assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_OK);
		assert_int_equal(canvas_px[4], 0xAB000005u);
		assert_int_equal(canvas_px[5], SENTINEL);

		/* The data stops inside a sub-block: the end of the data, unless it
		 * is complete. */
		frame_setup();
		len = frame_stream(idx, 5, false, 255) - 1;
		assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_ERR_END_OF_DATA);
		frame_setup();
		assert_int_equal(frame_decode(&canvas, &area, len, true), NSGIF_OK);
	}
}

/* Each LZW result the decode can see, as an nsgif_error: a bad code. */
static void frame_lzw_results(void **state)
{
	(void)state;
	struct nsgif_canvas canvas = {.bitmap = canvas_px, .width = 4, .height = 2, .rowspan = 4};

	for (int complex = 0; complex < 2; complex++) {
		struct nsgif_frame_area area = frame_area(0, 0, 4, 2);
		area.interlaced = complex;

		/* A code past the table after two pixels. */
		frame_setup();
		lzw_begin(&st, 8);
		lzw_clear(&st);
		lzw_code(&st, 1);
		lzw_code(&st, 2);
		lzw_put(&st, 300, st.width);
		frame_buf[0] = 8;
		size_t len = 1 + lzw_pack(&st, 255, frame_buf + 1);
		assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_ERR_DATA_FRAME);
		assert_int_equal(canvas_px[0], 0xAB000001u);
		assert_int_equal(canvas_px[1], 0xAB000002u);
		/* Complete data does not change a bad frame. */
		assert_int_equal(frame_decode(&canvas, &area, len, true), NSGIF_ERR_DATA_FRAME);

		/* A first code that is not a clear or a root. */
		frame_setup();
		lzw_begin(&st, 8);
		lzw_put(&st, 300, st.width);
		frame_buf[0] = 8;
		len = 1 + lzw_pack(&st, 255, frame_buf + 1);
		assert_int_equal(frame_decode(&canvas, &area, len, false), NSGIF_ERR_DATA_FRAME);
		assert_int_equal(canvas_px[0], SENTINEL);
	}
}

/* The restore before a frame: the whole image, or a frame's rectangle. */
static void frame_restore_bg(void **state)
{
	(void)state;
	/* Image 5x4 in a bitmap of 7 pixels per row. */
	struct nsgif_canvas canvas = {.bitmap = canvas_px,
				      .width = 5,
				      .height = 4,
				      .rowspan = 7,
				      .background = 0xFF112233u};

	/* The whole image: width * height pixels from the start to 0.
	 * Kept, characterized, not endorsed: by the image size, not the row
	 * span, so the last 2 * 4 pixels of the bitmap stay. */
	frame_setup();
	nsgif_frame_restore_bg(&canvas, NULL);
	for (size_t i = 0; i < 20; i++)
		assert_int_equal(canvas_px[i], 0);
	assert_int_equal(canvas_px[20], SENTINEL);

	/* A frame: its rectangle clipped to the image, to the background, rows
	 * stepped by the image width (also not the row span). */
	frame_setup();
	struct nsgif_frame_area area = frame_area(3, 2, 4, 3);
	nsgif_frame_restore_bg(&canvas, &area);
	assert_int_equal(canvas_px[2 * 5 + 3], 0xFF112233u);
	assert_int_equal(canvas_px[2 * 5 + 4], 0xFF112233u);
	assert_int_equal(canvas_px[3 * 5 + 3], 0xFF112233u);
	assert_int_equal(canvas_px[3 * 5 + 4], 0xFF112233u);
	assert_int_equal(canvas_px[2 * 5 + 5], SENTINEL);
	assert_int_equal(canvas_px[4 * 5 + 3], SENTINEL);
	assert_int_equal(canvas_px[2 * 5 + 2], SENTINEL);

	/* With transparency: to 0. */
	frame_setup();
	area.transparency = true;
	nsgif_frame_restore_bg(&canvas, &area);
	assert_int_equal(canvas_px[2 * 5 + 3], 0);
	assert_int_equal(canvas_px[3 * 5 + 4], 0);
	assert_int_equal(canvas_px[2 * 5 + 5], SENTINEL);

	/* A frame that is not displayed, or starts past the image: nothing. */
	static const uint32_t none[][4] = {{5, 0, 1, 1}, {0, 4, 1, 1}};
	for (size_t k = 0; k < 2; k++) {
		frame_setup();
		area = frame_area(none[k][0], none[k][1], none[k][2], none[k][3]);
		nsgif_frame_restore_bg(&canvas, &area);
		for (size_t i = 0; i < 28; i++)
			assert_int_equal(canvas_px[i], SENTINEL);
	}
	frame_setup();
	area = frame_area(0, 0, 2, 2);
	area.display = false;
	nsgif_frame_restore_bg(&canvas, &area);
	assert_int_equal(canvas_px[0], SENTINEL);
}

/* The recover: width * height pixels of the saved frame back, from the
 * start (also by the image size). */
static void frame_recover(void **state)
{
	(void)state;
	static uint32_t saved[28];
	for (uint32_t i = 0; i < 28; i++)
		saved[i] = 0xC0000000u | i;
	struct nsgif_canvas canvas = {.bitmap = canvas_px, .width = 5, .height = 4, .rowspan = 7};

	frame_setup();
	nsgif_frame_recover(&canvas, saved);
	for (uint32_t i = 0; i < 20; i++)
		assert_int_equal(canvas_px[i], 0xC0000000u | i);
	assert_int_equal(canvas_px[20], SENTINEL);
}

/* ---------------------------------------------------------------------- */
/* The state machine and the public API (gif.c) through nsgif.h. */

static uint8_t gb[1 << 16];
static size_t gb_len;

static void gb_u8(unsigned v)
{
	gb[gb_len++] = (uint8_t)v;
}

static void gb_u16(unsigned v)
{
	gb_u8(v & 0xff);
	gb_u8(v >> 8);
}

static void gb_put(const void *p, size_t n)
{
	memcpy(gb + gb_len, p, n);
	gb_len += n;
}

/* Colour i of every table the writer makes: (i * 16, 255 - i * 16, i * 8). */
static void gb_table(unsigned bits)
{
	for (unsigned i = 0; i < (2u << bits); i++) {
		gb_u8(i * 16);
		gb_u8(255 - i * 16);
		gb_u8(i * 8);
	}
}

/* That colour as an R8G8B8A8 pixel on a little-endian host. */
static uint32_t gb_rgba(unsigned i)
{
	return (i * 16) | ((255 - i * 16) << 8) | ((i * 8) << 16) | 0xFF000000u;
}

/* Header and logical screen descriptor, with a global colour table of
 * 2 << gct_bits entries when gct_bits >= 0. */
static void gb_begin(unsigned w, unsigned h, int gct_bits, unsigned bg)
{
	gb_len = 0;
	gb_put("GIF89a", 6);
	gb_u16(w);
	gb_u16(h);
	gb_u8(gct_bits >= 0 ? 0x80u | (unsigned)gct_bits : 0);
	gb_u8(bg);
	gb_u8(0);
	if (gct_bits >= 0)
		gb_table((unsigned)gct_bits);
}

static void gb_gce(unsigned disposal, unsigned delay, int transparent)
{
	gb_u8(0x21);
	gb_u8(0xF9);
	gb_u8(4);
	gb_u8((disposal << 2) | (transparent >= 0 ? 1 : 0));
	gb_u16(delay);
	gb_u8(transparent >= 0 ? (unsigned)transparent : 0);
	gb_u8(0);
}

static void gb_loops(unsigned count)
{
	gb_u8(0x21);
	gb_u8(0xFF);
	gb_u8(11);
	gb_put("NETSCAPE2.0", 11);
	gb_u8(3);
	gb_u8(1);
	gb_u16(count);
	gb_u8(0);
}

/* An image of one colour index, w * h pixels, with a local colour table
 * when lct_bits >= 0. */
static void gb_image(unsigned x, unsigned y, unsigned w, unsigned h, bool interlaced, int lct_bits, unsigned index)
{
	gb_u8(0x2C);
	gb_u16(x);
	gb_u16(y);
	gb_u16(w);
	gb_u16(h);
	gb_u8((lct_bits >= 0 ? 0x80u | (unsigned)lct_bits : 0) | (interlaced ? 0x40u : 0));
	if (lct_bits >= 0)
		gb_table((unsigned)lct_bits);
	lzw_begin(&st, 8);
	lzw_clear(&st);
	for (unsigned i = 0; i < w * h; i++)
		lzw_code(&st, index);
	lzw_eoi(&st);
	gb_u8(8);
	gb_len += lzw_pack(&st, 255, gb + gb_len);
}

static void gb_end(void)
{
	gb_u8(0x3B);
}

/* The three-frame GIF of most cases: 4x4, four global colours, background
 * colour 3, loop count `loops` (none when negative).
 * Frame 0: the whole image in colour 1, delay 5, disposal none.
 * Frame 1: 2x2 at (1, 1) in colour 2, delay 7, disposal `disposal1`,
 *          transparent index `transparent1` (none when negative).
 * Frame 2: 1x1 at (0, 0) in colour 0 of a two-entry local table, no
 *          graphic control extension. */
static void gb_three_frames(int loops, unsigned disposal1, int transparent1)
{
	gb_begin(4, 4, 1, 3);
	if (loops >= 0)
		gb_loops((unsigned)loops);
	gb_gce(NSGIF_DISPOSAL_NONE, 5, -1);
	gb_image(0, 0, 4, 4, false, -1, 1);
	gb_gce(disposal1, 7, transparent1);
	gb_image(1, 1, 2, 2, false, -1, 2);
	gb_image(0, 0, 1, 1, false, 0, 0);
	gb_end();
}

static nsgif_t *gif_scan_all(const nsgif_bitmap_cb_vt *vt, nsgif_bitmap_fmt_t fmt, bool complete)
{
	nsgif_t *gif;
	assert_int_equal(nsgif_create(vt, fmt, &gif), NSGIF_OK);
	assert_int_equal(nsgif_data_scan(gif, gb_len, gb), NSGIF_OK);
	if (complete)
		nsgif_data_complete(gif);
	return gif;
}

static void gif_strings(void **state)
{
	(void)state;
	static const char *const errors[] = {
		"Success",
		"Out of memory",
		"Invalid source data",
		"Requested frame does not exist",
		"Invalid frame data",
		"Unexpected end of GIF source data",
		"Can't add data to completed GIF",
		"Frame can't be displayed",
		"Animation complete",
	};
	for (int i = 0; i < 9; i++)
		assert_string_equal(nsgif_strerror((nsgif_error)i), errors[i]);
	assert_string_equal(nsgif_strerror((nsgif_error)9), "Unknown error");
	assert_string_equal(nsgif_strerror((nsgif_error)1000), "Unknown error");

	static const char *const disposals[] = {
		"Unspecified", "None", "Restore background", "Restore previous", "Restore quirk",
	};
	for (int i = 0; i < 5; i++)
		assert_string_equal(nsgif_str_disposal((enum nsgif_disposal)i), disposals[i]);
	assert_string_equal(nsgif_str_disposal((enum nsgif_disposal)5), "Unspecified");
	assert_string_equal(nsgif_str_disposal((enum nsgif_disposal)200), "Unspecified");
}

/* The bitmap format sets the byte order of every palette and pixel; the
 * host-order formats map to the byte orders of a little-endian host. */
static void gif_create_and_layouts(void **state)
{
	(void)state;
	static const struct {
		nsgif_bitmap_fmt_t fmt;
		uint8_t bytes[4]; /* colour 1: r 16, g 239, b 8, a 255 */
	} cases[] = {
		{NSGIF_BITMAP_FMT_R8G8B8A8, {16, 239, 8, 255}}, {NSGIF_BITMAP_FMT_B8G8R8A8, {8, 239, 16, 255}},
		{NSGIF_BITMAP_FMT_A8R8G8B8, {255, 16, 239, 8}}, {NSGIF_BITMAP_FMT_A8B8G8R8, {255, 8, 239, 16}},
		{NSGIF_BITMAP_FMT_RGBA8888, {255, 8, 239, 16}}, {NSGIF_BITMAP_FMT_BGRA8888, {255, 16, 239, 8}},
		{NSGIF_BITMAP_FMT_ARGB8888, {8, 239, 16, 255}}, {NSGIF_BITMAP_FMT_ABGR8888, {16, 239, 8, 255}},
		{(nsgif_bitmap_fmt_t)99, {16, 239, 8, 255}},
	};
	gb_three_frames(-1, NSGIF_DISPOSAL_NONE, -1);
	for (size_t k = 0; k < sizeof(cases) / sizeof(cases[0]); k++) {
		nsgif_t *gif = gif_scan_all(&bitmap_vt, cases[k].fmt, false);
		uint32_t table[NSGIF_MAX_COLOURS];
		size_t entries = 0;
		nsgif_global_palette(gif, table, &entries);
		assert_int_equal(entries, 4);
		assert_memory_equal(&table[1], cases[k].bytes, 4);
		nsgif_destroy(gif);
	}

	/* Without a global colour table: black and white, two entries. */
	gb_begin(2, 1, -1, 0);
	gb_image(0, 0, 2, 1, false, -1, 1);
	gb_end();
	nsgif_t *gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, false);
	uint32_t table[NSGIF_MAX_COLOURS];
	size_t entries = 0;
	nsgif_global_palette(gif, table, &entries);
	assert_int_equal(entries, 2);
	assert_int_equal(table[0], 0xFF000000u);
	assert_int_equal(table[1], 0xFFFFFFFFu);
	assert_false(nsgif_get_info(gif)->global_palette);
	assert_int_equal(nsgif_get_info(gif)->background, 0xFF000000u);
	nsgif_destroy(gif);

	/* nsgif_destroy takes NULL. */
	nsgif_destroy(NULL);
}

static void gif_scan_whole(void **state)
{
	(void)state;
	gb_three_frames(2, NSGIF_DISPOSAL_RESTORE_BG, -1);
	nsgif_t *gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, false);
	const nsgif_info_t *info = nsgif_get_info(gif);
	assert_int_equal(info->width, 4);
	assert_int_equal(info->height, 4);
	assert_int_equal(info->frame_count, 3);
	assert_int_equal(info->loop_max, 3);
	assert_true(info->global_palette);
	assert_int_equal(info->background, gb_rgba(3));

	static const struct {
		uint32_t delay, disposal, x0, y0, x1, y1;
		bool local_palette;
	} frames[] = {
		{5, NSGIF_DISPOSAL_NONE, 0, 0, 4, 4, false},
		{7, NSGIF_DISPOSAL_RESTORE_BG, 1, 1, 3, 3, false},
		{10, NSGIF_DISPOSAL_UNSPECIFIED, 0, 0, 1, 1, true},
	};
	for (uint32_t f = 0; f < 3; f++) {
		const nsgif_frame_info_t *fi = nsgif_get_frame_info(gif, f);
		assert_non_null(fi);
		assert_true(fi->display);
		assert_false(fi->transparency);
		assert_false(fi->interlaced);
		assert_int_equal(fi->delay, frames[f].delay);
		assert_int_equal(fi->disposal, frames[f].disposal);
		assert_int_equal(fi->rect.x0, frames[f].x0);
		assert_int_equal(fi->rect.y0, frames[f].y0);
		assert_int_equal(fi->rect.x1, frames[f].x1);
		assert_int_equal(fi->rect.y1, frames[f].y1);
		assert_int_equal(fi->local_palette, frames[f].local_palette);
	}
	assert_null(nsgif_get_frame_info(gif, 3));

	uint32_t table[NSGIF_MAX_COLOURS];
	size_t entries = 0;
	assert_false(nsgif_local_palette(gif, 0, table, &entries));
	assert_true(nsgif_local_palette(gif, 2, table, &entries));
	assert_int_equal(entries, 2);
	assert_int_equal(table[1], gb_rgba(1));
	assert_false(nsgif_local_palette(gif, 3, table, &entries));

	/* Scanning again with the same data finds nothing new. */
	assert_int_equal(nsgif_data_scan(gif, gb_len, gb), NSGIF_OK);
	assert_int_equal(info->frame_count, 3);
	/* After nsgif_data_complete, no more data. */
	nsgif_data_complete(gif);
	assert_int_equal(nsgif_data_scan(gif, gb_len, gb), NSGIF_ERR_DATA_COMPLETE);
	nsgif_destroy(gif);
}

/* Field by field: the padding of the structs is not part of the API. */
static void assert_info_equal(const nsgif_info_t *a, const nsgif_info_t *b)
{
	assert_int_equal(a->width, b->width);
	assert_int_equal(a->height, b->height);
	assert_int_equal(a->frame_count, b->frame_count);
	assert_int_equal(a->loop_max, b->loop_max);
	assert_int_equal(a->background, b->background);
	assert_int_equal(a->global_palette, b->global_palette);
}

static void assert_frame_info_equal(const nsgif_frame_info_t *a, const nsgif_frame_info_t *b)
{
	assert_int_equal(a->display, b->display);
	assert_int_equal(a->transparency, b->transparency);
	assert_int_equal(a->local_palette, b->local_palette);
	assert_int_equal(a->interlaced, b->interlaced);
	assert_int_equal(a->disposal, b->disposal);
	assert_int_equal(a->delay, b->delay);
	assert_memory_equal(&a->rect, &b->rect, sizeof(a->rect));
}

/* Scans in parts, as data arrives. The writer's three-frame GIF puts
 * frame 0's graphic control extension at bytes 44 to 51, its image
 * descriptor at 52 to 61 and its image data from 62.
 *
 * Kept, characterized, not endorsed: when a scan stops at the end of the
 * data inside a frame, nsgif_data_scan keeps the position where the frame's
 * parse stopped, not the frame's start. The next scan resumes there and
 * reads image data (or an extension's terminator) as the next block, which
 * is a bad frame from then on, also with all the data. A split before the
 * extension's last byte or inside the image descriptor resumes correctly. */
static void gif_scan_incremental(void **state)
{
	(void)state;
	gb_three_frames(2, NSGIF_DISPOSAL_RESTORE_BG, -1);
	nsgif_t *whole = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, false);
	nsgif_t *gif;

	/* Split points that resume correctly: the end state of a whole scan. */
	static const size_t good[] = {13, 25, 44, 50, 52, 61};
	for (size_t k = 0; k < sizeof(good) / sizeof(good[0]); k++) {
		assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
		assert_int_equal(nsgif_data_scan(gif, good[k], gb), NSGIF_ERR_END_OF_DATA);
		assert_int_equal(nsgif_data_scan(gif, gb_len, gb), NSGIF_OK);
		assert_info_equal(nsgif_get_info(gif), nsgif_get_info(whole));
		for (uint32_t f = 0; f < 3; f++)
			assert_frame_info_equal(nsgif_get_frame_info(gif, f), nsgif_get_frame_info(whole, f));
		nsgif_destroy(gif);
	}

	/* Split points inside frame 0's image data, or on its extension's
	 * last byte: a bad frame for good. */
	static const size_t bad[] = {51, 62, 70, 85};
	for (size_t k = 0; k < sizeof(bad) / sizeof(bad[0]); k++) {
		assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
		assert_int_equal(nsgif_data_scan(gif, bad[k], gb), NSGIF_ERR_END_OF_DATA);
		assert_int_equal(nsgif_data_scan(gif, gb_len, gb), NSGIF_ERR_DATA_FRAME);
		assert_int_equal(nsgif_data_scan(gif, gb_len, gb), NSGIF_ERR_DATA_FRAME);
		assert_int_equal(nsgif_get_info(gif)->frame_count, 0);
		nsgif_destroy(gif);
	}

	/* Byte by byte: the end of the data up to byte 60, then a bad frame. */
	assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
	for (size_t n = 0; n <= gb_len; n++) {
		nsgif_error ret = nsgif_data_scan(gif, n, gb);
		assert_int_equal(ret, n < 61 ? NSGIF_ERR_END_OF_DATA : NSGIF_ERR_DATA_FRAME);
		assert_int_equal(nsgif_get_info(gif)->frame_count, 0);
	}
	nsgif_destroy(gif);
	nsgif_destroy(whole);
}

/* Reported screen sizes from the quirk list become 1x1, and the first frame
 * sets the size; a first frame larger than the screen grows it; later
 * frames do not. */
static void gif_screen_size(void **state)
{
	(void)state;
	static const unsigned screens[][2] = {{640, 480}, {0, 0}, {2049, 5}};
	for (size_t k = 0; k < 3; k++) {
		gb_begin(screens[k][0], screens[k][1], 1, 0);
		gb_image(0, 0, 3, 2, false, -1, 1);
		gb_end();
		nsgif_t *gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, false);
		assert_int_equal(nsgif_get_info(gif)->width, 3);
		assert_int_equal(nsgif_get_info(gif)->height, 2);
		nsgif_destroy(gif);
	}

	gb_begin(4, 4, 1, 0);
	gb_image(0, 0, 6, 5, false, -1, 1);
	gb_image(0, 0, 9, 9, false, -1, 1);
	gb_end();
	nsgif_t *gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, false);
	assert_int_equal(nsgif_get_info(gif)->width, 6);
	assert_int_equal(nsgif_get_info(gif)->height, 5);
	nsgif_destroy(gif);
}

/* A GIF with no frames: a header, a screen and a trailer scans; nothing can
 * be displayed or decoded. Without the trailer it is the end of the data. */
static void gif_without_frames(void **state)
{
	(void)state;
	gb_begin(4, 4, -1, 0);
	gb_end();
	nsgif_t *gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, true);
	assert_int_equal(nsgif_get_info(gif)->frame_count, 0);
	nsgif_bitmap_t *bitmap = NULL;
	assert_int_equal(nsgif_frame_decode(gif, 0, &bitmap), NSGIF_ERR_BAD_FRAME);
	nsgif_rect_t area;
	uint32_t delay, frame;
	assert_int_equal(nsgif_frame_prepare(gif, &area, &delay, &frame), NSGIF_ERR_FRAME_DISPLAY);
	nsgif_destroy(gif);

	assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
	assert_int_equal(nsgif_data_scan(gif, gb_len - 1, gb), NSGIF_ERR_END_OF_DATA);
	nsgif_destroy(gif);

	/* Image data that is only a minimum code size and the trailer: no
	 * frame and no error. */
	gb_begin(4, 4, -1, 0);
	gb_u8(0x2C);
	gb_u16(0);
	gb_u16(0);
	gb_u16(2);
	gb_u16(2);
	gb_u8(0);
	gb_u8(8);
	gb_u8(0x3B);
	assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
	assert_int_equal(nsgif_data_scan(gif, gb_len, gb), NSGIF_OK);
	assert_int_equal(nsgif_get_info(gif)->frame_count, 0);
	nsgif_destroy(gif);

	/* A minimum code size of 12 (LZW_CODE_MAX): a bad frame. */
	gb_begin(4, 4, -1, 0);
	gb_u8(0x2C);
	gb_u16(0);
	gb_u16(0);
	gb_u16(2);
	gb_u16(2);
	gb_u8(0);
	gb_u8(12);
	gb_u8(1);
	gb_u8(0);
	gb_u8(0);
	gb_end();
	assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
	assert_int_equal(nsgif_data_scan(gif, gb_len, gb), NSGIF_ERR_DATA_FRAME);
	assert_int_equal(nsgif_get_info(gif)->frame_count, 0);
	nsgif_destroy(gif);

	/* Not a GIF. */
	assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
	assert_int_equal(nsgif_data_scan(gif, 13, (const uint8_t *)"PNG89a\0\0\0\0\0\0\0"), NSGIF_ERR_DATA);
	nsgif_destroy(gif);
}

/* nsgif_data_complete makes the first frame with image data that the scan
 * could not finish displayable; a first frame gets transparency. */
static void gif_data_complete_partial(void **state)
{
	(void)state;
	gb_three_frames(-1, NSGIF_DISPOSAL_NONE, -1);
	size_t full = gb_len;

	/* Cut inside frame 1's image data (its last sub-block, before the
	 * frame 2 extension and image). */
	size_t cut = 0;
	{
		/* A new scan for every length (not one scan in parts, see
		 * gif_scan_incremental). */
		for (size_t n = 0; n <= full && !cut; n++) {
			nsgif_t *gif;
			assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
			(void)nsgif_data_scan(gif, n, gb);
			if (nsgif_get_info(gif)->frame_count == 2)
				cut = n - 2;
			nsgif_destroy(gif);
		}
	}
	assert_true(cut > 0);

	nsgif_t *gif;
	assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
	assert_int_equal(nsgif_data_scan(gif, cut, gb), NSGIF_OK);
	assert_int_equal(nsgif_get_info(gif)->frame_count, 1);
	assert_null(nsgif_get_frame_info(gif, 1));
	nsgif_data_complete(gif);
	assert_int_equal(nsgif_get_info(gif)->frame_count, 2);
	assert_true(nsgif_get_frame_info(gif, 1)->display);
	assert_false(nsgif_get_frame_info(gif, 1)->transparency);
	nsgif_bitmap_t *bitmap = NULL;
	assert_int_equal(nsgif_frame_decode(gif, 1, &bitmap), NSGIF_OK);
	nsgif_destroy(gif);

	/* Cut inside frame 0: nothing scanned; complete makes frame 0
	 * displayable with transparency, and it decodes as far as it goes. */
	assert_int_equal(nsgif_create(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, &gif), NSGIF_OK);
	size_t cut0 = 13 + 12 + 19 + 8 + 10 + 3;
	assert_int_equal(nsgif_data_scan(gif, cut0, gb), NSGIF_ERR_END_OF_DATA);
	assert_int_equal(nsgif_get_info(gif)->frame_count, 0);
	nsgif_data_complete(gif);
	assert_int_equal(nsgif_get_info(gif)->frame_count, 1);
	assert_true(nsgif_get_frame_info(gif, 0)->display);
	assert_true(nsgif_get_frame_info(gif, 0)->transparency);
	assert_int_equal(nsgif_frame_decode(gif, 0, &bitmap), NSGIF_OK);
	nsgif_destroy(gif);
}

/* The frame order of nsgif_frame_prepare: the redraw area, the delay rules
 * (below the minimum becomes the default), the loop count and the last
 * frame's infinite delay. */
static void gif_prepare_sequence(void **state)
{
	(void)state;
	nsgif_rect_t area;
	uint32_t delay, frame;

	/* No loop extension: loop_max 1, play once. */
	gb_three_frames(-1, NSGIF_DISPOSAL_NONE, -1);
	nsgif_t *gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, true);
	assert_int_equal(nsgif_get_info(gif)->loop_max, 1);
	static const struct {
		uint32_t frame, delay, x0, y0, x1, y1;
	} once[] = {
		{0, 5, 0, 0, 4, 4},
		{1, 7, 0, 0, 4, 4},
		{2, NSGIF_INFINITE, 0, 0, 3, 3},
		/* Kept, characterized, not endorsed: one more call shows frame 0
		 * again before the end. */
		{0, 5, 0, 0, 4, 4},
	};
	for (size_t k = 0; k < 4; k++) {
		assert_int_equal(nsgif_frame_prepare(gif, &area, &delay, &frame), NSGIF_OK);
		assert_int_equal(frame, once[k].frame);
		assert_int_equal(delay, once[k].delay);
		assert_int_equal(area.x0, once[k].x0);
		assert_int_equal(area.y0, once[k].y0);
		assert_int_equal(area.x1, once[k].x1);
		assert_int_equal(area.y1, once[k].y1);
	}
	assert_int_equal(nsgif_frame_prepare(gif, &area, &delay, &frame), NSGIF_ERR_ANIMATION_END);

	/* nsgif_reset starts over. */
	assert_int_equal(nsgif_reset(gif), NSGIF_OK);
	assert_int_equal(nsgif_frame_prepare(gif, &area, &delay, &frame), NSGIF_OK);
	assert_int_equal(frame, 0);

	/* The delay rules: below delay_min, delay_default. */
	nsgif_set_frame_delay_behaviour(gif, 6, 50);
	assert_int_equal(nsgif_reset(gif), NSGIF_OK);
	assert_int_equal(nsgif_frame_prepare(gif, &area, &delay, &frame), NSGIF_OK);
	assert_int_equal(delay, 50);
	assert_int_equal(nsgif_frame_prepare(gif, &area, &delay, &frame), NSGIF_OK);
	assert_int_equal(delay, 7);
	nsgif_destroy(gif);

	/* Loop count 0: forever, no infinite delay. */
	gb_three_frames(0, NSGIF_DISPOSAL_NONE, -1);
	gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, true);
	assert_int_equal(nsgif_get_info(gif)->loop_max, 0);
	for (int k = 0; k < 10; k++) {
		assert_int_equal(nsgif_frame_prepare(gif, &area, &delay, &frame), NSGIF_OK);
		assert_int_equal(frame, (uint32_t)(k % 3));
		assert_true(delay != NSGIF_INFINITE);
	}
	nsgif_destroy(gif);

	/* Data not complete: after the last scanned frame, the end of the
	 * data. */
	gb_three_frames(0, NSGIF_DISPOSAL_NONE, -1);
	gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, false);
	for (int k = 0; k < 3; k++)
		assert_int_equal(nsgif_frame_prepare(gif, &area, &delay, &frame), NSGIF_OK);
	assert_int_equal(nsgif_frame_prepare(gif, &area, &delay, &frame), NSGIF_ERR_END_OF_DATA);
	nsgif_destroy(gif);

	/* One frame: an infinite delay. */
	gb_begin(2, 2, 1, 0);
	gb_image(0, 0, 2, 2, false, -1, 1);
	gb_end();
	gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, true);
	assert_int_equal(nsgif_frame_prepare(gif, &area, &delay, &frame), NSGIF_OK);
	assert_int_equal(delay, NSGIF_INFINITE);
	nsgif_destroy(gif);
}

static const uint32_t *gif_pixels(nsgif_bitmap_t *bitmap)
{
	return (const uint32_t *)bitmap_get_buffer(bitmap);
}

/* The disposal of the previous frame before a decode, frames decoded in
 * order from a restart, and a cached frame. */
static void gif_decode_disposal(void **state)
{
	(void)state;
	nsgif_bitmap_t *bitmap = NULL;

	/* Restore background: frame 1's rectangle back to the background
	 * colour (3), then frame 2 at (0, 0). */
	gb_three_frames(-1, NSGIF_DISPOSAL_RESTORE_BG, -1);
	nsgif_t *gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, true);
	assert_int_equal(nsgif_frame_decode(gif, 2, &bitmap), NSGIF_OK);
	const uint32_t *px = gif_pixels(bitmap);
	for (uint32_t y = 0; y < 4; y++) {
		for (uint32_t x = 0; x < 4; x++) {
			uint32_t expect = gb_rgba(1);
			if (x >= 1 && x < 3 && y >= 1 && y < 3)
				expect = gb_rgba(3);
			if (x == 0 && y == 0)
				expect = gb_rgba(0);
			assert_int_equal(px[y * 4 + x], expect);
		}
	}
	/* Out of order: frame 1 restarts from frame 0. */
	assert_int_equal(nsgif_frame_decode(gif, 1, &bitmap), NSGIF_OK);
	px = gif_pixels(bitmap);
	assert_int_equal(px[0], gb_rgba(1));
	assert_int_equal(px[1 * 4 + 1], gb_rgba(2));
	/* The same frame again: the cached bitmap. */
	nsgif_bitmap_t *again = NULL;
	assert_int_equal(nsgif_frame_decode(gif, 1, &again), NSGIF_OK);
	assert_ptr_equal(again, bitmap);
	assert_int_equal(nsgif_frame_decode(gif, 3, &again), NSGIF_ERR_BAD_FRAME);
	nsgif_destroy(gif);

	/* Restore background with a transparent index: back to 0. */
	gb_three_frames(-1, NSGIF_DISPOSAL_RESTORE_BG, 9);
	gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, true);
	assert_int_equal(nsgif_frame_decode(gif, 2, &bitmap), NSGIF_OK);
	px = gif_pixels(bitmap);
	assert_int_equal(px[1 * 4 + 1], 0);
	assert_int_equal(px[2 * 4 + 2], 0);
	assert_int_equal(px[3 * 4 + 3], gb_rgba(1));
	nsgif_destroy(gif);

	/* Restore previous: frame 1's area as before frame 1 (colour 1). */
	gb_three_frames(-1, NSGIF_DISPOSAL_RESTORE_PREV, -1);
	gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, true);
	assert_int_equal(nsgif_frame_decode(gif, 1, &bitmap), NSGIF_OK);
	assert_int_equal(gif_pixels(bitmap)[1 * 4 + 1], gb_rgba(2));
	assert_int_equal(nsgif_frame_decode(gif, 2, &bitmap), NSGIF_OK);
	px = gif_pixels(bitmap);
	assert_int_equal(px[0], gb_rgba(0));
	assert_int_equal(px[1 * 4 + 1], gb_rgba(1));
	assert_int_equal(px[2 * 4 + 2], gb_rgba(1));
	nsgif_destroy(gif);

	/* Frame 1's own transparent pixels leave frame 0. */
	gb_three_frames(-1, NSGIF_DISPOSAL_NONE, 2);
	gif = gif_scan_all(&bitmap_vt, NSGIF_BITMAP_FMT_R8G8B8A8, true);
	assert_int_equal(nsgif_frame_decode(gif, 1, &bitmap), NSGIF_OK);
	assert_int_equal(gif_pixels(bitmap)[1 * 4 + 1], gb_rgba(1));
	nsgif_destroy(gif);
}

/* The optional callbacks: when each is called, and a row span wider than
 * the image. */
static int cb_create, cb_modified, cb_test_opaque, cb_set_opaque;
static bool cb_last_opaque;
static uint32_t cb_extra;

static nsgif_bitmap_t *cb_bitmap_create(int width, int height)
{
	cb_create++;
	return calloc(((size_t)width + cb_extra) * (size_t)height, 4);
}

static void cb_bitmap_modified(nsgif_bitmap_t *bitmap)
{
	(void)bitmap;
	cb_modified++;
}

static bool cb_bitmap_test_opaque(nsgif_bitmap_t *bitmap)
{
	(void)bitmap;
	cb_test_opaque++;
	return true;
}

static void cb_bitmap_set_opaque(nsgif_bitmap_t *bitmap, bool opaque)
{
	(void)bitmap;
	cb_set_opaque++;
	cb_last_opaque = opaque;
}

static uint32_t cb_bitmap_rowspan(nsgif_bitmap_t *bitmap)
{
	(void)bitmap;
	return 4 + cb_extra;
}

static void gif_callbacks(void **state)
{
	(void)state;
	const nsgif_bitmap_cb_vt vt = {
		.create = cb_bitmap_create,
		.destroy = bitmap_destroy,
		.get_buffer = bitmap_get_buffer,
		.set_opaque = cb_bitmap_set_opaque,
		.test_opaque = cb_bitmap_test_opaque,
		.modified = cb_bitmap_modified,
		.get_rowspan = cb_bitmap_rowspan,
	};
	cb_create = cb_modified = cb_test_opaque = cb_set_opaque = 0;
	cb_extra = 3;
	gb_three_frames(-1, NSGIF_DISPOSAL_NONE, -1);
	nsgif_t *gif = gif_scan_all(&vt, NSGIF_BITMAP_FMT_R8G8B8A8, true);
	nsgif_bitmap_t *bitmap = NULL;

	assert_int_equal(nsgif_frame_decode(gif, 0, &bitmap), NSGIF_OK);
	assert_int_equal(cb_create, 1);
	assert_int_equal(cb_modified, 1);
	assert_int_equal(cb_test_opaque, 1);
	assert_int_equal(cb_set_opaque, 1);
	assert_true(cb_last_opaque);
	/* Rows step by the row span (7). */
	const uint32_t *px = gif_pixels(bitmap);
	assert_int_equal(px[0], gb_rgba(1));
	assert_int_equal(px[3], gb_rgba(1));
	assert_int_equal(px[7], gb_rgba(1));
	assert_int_equal(px[3 * 7 + 3], gb_rgba(1));

	assert_int_equal(nsgif_frame_decode(gif, 1, &bitmap), NSGIF_OK);
	assert_int_equal(cb_modified, 2);
	assert_int_equal(cb_test_opaque, 2);
	assert_int_equal(cb_set_opaque, 2);
	assert_int_equal(gif_pixels(bitmap)[1 * 7 + 1], gb_rgba(2));

	/* Frame 0 again: a restart; frame 0 was decoded before, so its opacity
	 * is not tested again, only set. */
	assert_int_equal(nsgif_frame_decode(gif, 0, &bitmap), NSGIF_OK);
	assert_int_equal(cb_create, 1);
	assert_int_equal(cb_modified, 3);
	assert_int_equal(cb_test_opaque, 2);
	assert_int_equal(cb_set_opaque, 3);
	nsgif_destroy(gif);
	cb_extra = 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(lzw_code_on_byte_boundary_at_end_of_data),
		cmocka_unit_test(lzw_roots_at_every_code_size),
		cmocka_unit_test(lzw_kwkwk_and_clear),
		cmocka_unit_test(lzw_bad_codes),
		cmocka_unit_test(lzw_end_of_data),
		cmocka_unit_test(lzw_codes_across_sub_blocks),
		cmocka_unit_test(lzw_eoi_on_byte_boundary),
		cmocka_unit_test(lzw_full_table),
		cmocka_unit_test(lzw_output_stack_full),
		cmocka_unit_test(lzw_decode_map_colours_and_transparency),
		cmocka_unit_test(lzw_decode_map_needs_a_colour_table),
		cmocka_unit_test(blocks_header),
		cmocka_unit_test(blocks_screen),
		cmocka_unit_test(blocks_screen_size_quirk),
		cmocka_unit_test(blocks_colour_table),
		cmocka_unit_test(blocks_graphic_control),
		cmocka_unit_test(blocks_graphic_control_short),
		cmocka_unit_test(blocks_loop_count),
		cmocka_unit_test(blocks_skip_extensions),
		cmocka_unit_test(blocks_image_descriptor),
		cmocka_unit_test(frame_simple_full_frame),
		cmocka_unit_test(frame_complex_offset_and_clip),
		cmocka_unit_test(frame_interlaced_heights),
		cmocka_unit_test(frame_transparency),
		cmocka_unit_test(frame_outside_the_image),
		cmocka_unit_test(frame_end_of_data),
		cmocka_unit_test(frame_lzw_results),
		cmocka_unit_test(frame_restore_bg),
		cmocka_unit_test(frame_recover),
		cmocka_unit_test(gif_strings),
		cmocka_unit_test(gif_create_and_layouts),
		cmocka_unit_test(gif_scan_whole),
		cmocka_unit_test(gif_scan_incremental),
		cmocka_unit_test(gif_screen_size),
		cmocka_unit_test(gif_without_frames),
		cmocka_unit_test(gif_data_complete_partial),
		cmocka_unit_test(gif_prepare_sequence),
		cmocka_unit_test(gif_decode_disposal),
		cmocka_unit_test(gif_callbacks),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
