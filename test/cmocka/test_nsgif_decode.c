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
 * test_nsgif_decode_rust builds the same cases with NSGIF_TEST_RUST_LZW:
 * without lzw.c, against the Rust shims of obs-graphics (lzw.h is the
 * boundary; gif.c stays C here too).
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
#ifndef NSGIF_TEST_RUST_LZW
#include "graphics/libnsgif/lzw.c.h"
#endif
#include "graphics/libnsgif/gif.c.h"
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
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
