/* Characterize obs_source_frame_copy() (copy_frame_data()) through the
 * exported function, before its Rust replacement (#5714). Every frame pair
 * has the same format, width and height, as libobs's own callers pass:
 * the source comes from obs_source_frame_create(), the destination either
 * from obs_source_frame_create() too (equal line sizes) or built here with
 * its own line sizes per plane. No OBS core, video device or graphics
 * context is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>
#include "obs.h"

#define FILL_SRC(plane, i) ((uint8_t)((plane) * 61u + (i) * 7u + 3u))
#define FILL_DST 0xEE
#define PLANES 4

/* The planes obs_source_frame_create() gives each format, and how many of
 * them copy_frame_data() copies, with the rows of each ((height + 1) / 2
 * for 4:2:0 chroma). Characterized, not endorsed: P216 and P416 copy no
 * plane. */
struct format_case {
	enum video_format format;
	size_t planes;
	size_t copied;
	bool half[PLANES];
};

static const struct format_case formats[] = {
	{VIDEO_FORMAT_I420, 3, 3, {false, true, true, false}},
	{VIDEO_FORMAT_NV12, 2, 2, {false, true, false, false}},
	{VIDEO_FORMAT_YVYU, 1, 1, {false}},
	{VIDEO_FORMAT_YUY2, 1, 1, {false}},
	{VIDEO_FORMAT_UYVY, 1, 1, {false}},
	{VIDEO_FORMAT_RGBA, 1, 1, {false}},
	{VIDEO_FORMAT_BGRA, 1, 1, {false}},
	{VIDEO_FORMAT_BGRX, 1, 1, {false}},
	{VIDEO_FORMAT_Y800, 1, 1, {false}},
	{VIDEO_FORMAT_I444, 3, 3, {false}},
	{VIDEO_FORMAT_BGR3, 1, 1, {false}},
	{VIDEO_FORMAT_I422, 3, 3, {false}},
	{VIDEO_FORMAT_I40A, 4, 4, {false, true, true, false}},
	{VIDEO_FORMAT_I42A, 4, 4, {false}},
	{VIDEO_FORMAT_YUVA, 4, 4, {false}},
	{VIDEO_FORMAT_AYUV, 1, 1, {false}},
	{VIDEO_FORMAT_I010, 3, 3, {false, true, true, false}},
	{VIDEO_FORMAT_P010, 2, 2, {false, true, false, false}},
	{VIDEO_FORMAT_I210, 3, 3, {false}},
	{VIDEO_FORMAT_I412, 3, 3, {false}},
	{VIDEO_FORMAT_YA2L, 4, 4, {false}},
	{VIDEO_FORMAT_P216, 2, 0, {false}},
	{VIDEO_FORMAT_P416, 2, 0, {false}},
	{VIDEO_FORMAT_V210, 1, 1, {false}},
	{VIDEO_FORMAT_R10L, 1, 1, {false}},
};

#define FORMAT_COUNT (sizeof(formats) / sizeof(formats[0]))

static const uint32_t sizes[][2] = {{1, 1}, {13, 7}, {32, 16}, {5, 2}};

#define SIZE_COUNT (sizeof(sizes) / sizeof(sizes[0]))

static uint32_t plane_rows(const struct format_case *fc, size_t plane, uint32_t height)
{
	return fc->half[plane] ? (height + 1) / 2 : height;
}

static void fill_source(struct obs_source_frame *src, const struct format_case *fc)
{
	for (size_t p = 0; p < fc->planes; p++) {
		size_t len = (size_t)src->linesize[p] * plane_rows(fc, p, src->height);
		for (size_t i = 0; i < len; i++)
			src->data[p][i] = FILL_SRC(p, i);
	}
}

/* A destination of the source's format and size with its own line size per
 * plane, each plane exactly line size x rows, filled with FILL_DST. */
static void build_destination(struct obs_source_frame *dst, const struct format_case *fc, uint32_t width,
			      uint32_t height, const uint32_t *linesize)
{
	memset(dst, 0, sizeof(*dst));
	dst->format = fc->format;
	dst->width = width;
	dst->height = height;
	for (size_t p = 0; p < fc->planes; p++) {
		size_t len = (size_t)linesize[p] * plane_rows(fc, p, height);
		dst->linesize[p] = linesize[p];
		dst->data[p] = malloc(len);
		assert_non_null(dst->data[p]);
		memset(dst->data[p], FILL_DST, len);
	}
}

static void free_destination(struct obs_source_frame *dst)
{
	for (size_t p = 0; p < MAX_AV_PLANES; p++)
		free(dst->data[p]);
}

/* Row y of every copied plane holds the source's row y up to the smaller
 * line size; the rest of a longer destination row is untouched. Planes that
 * are not copied are untouched. */
static void check_planes(const struct obs_source_frame *dst, const struct obs_source_frame *src,
			 const struct format_case *fc)
{
	for (size_t p = 0; p < fc->planes; p++) {
		uint32_t rows = plane_rows(fc, p, dst->height);
		uint32_t dst_line = dst->linesize[p];
		uint32_t src_line = src->linesize[p];
		uint32_t bytes = dst_line < src_line ? dst_line : src_line;
		for (uint32_t y = 0; y < rows; y++) {
			const uint8_t *row = dst->data[p] + (size_t)y * dst_line;
			const uint8_t *from = src->data[p] + (size_t)y * src_line;
			for (uint32_t x = 0; x < dst_line; x++) {
				uint8_t want = (p < fc->copied && x < bytes) ? from[x] : FILL_DST;
				if (row[x] != want) {
					fprintf(stderr, "format %d %ux%u plane %zu row %u byte %u: got %u, want %u\n",
						(int)fc->format, dst->width, dst->height, p, y, x, row[x], want);
					fail();
				}
			}
		}
	}
}

static void check_equal_line_sizes(const struct format_case *fc, uint32_t width, uint32_t height)
{
	struct obs_source_frame *src = obs_source_frame_create(fc->format, width, height);
	struct obs_source_frame *dst = obs_source_frame_create(fc->format, width, height);
	assert_non_null(src);
	assert_non_null(dst);
	fill_source(src, fc);
	for (size_t p = 0; p < fc->planes; p++) {
		assert_int_equal(dst->linesize[p], src->linesize[p]);
		memset(dst->data[p], FILL_DST, (size_t)dst->linesize[p] * plane_rows(fc, p, height));
	}

	obs_source_frame_copy(dst, src);
	check_planes(dst, src, fc);

	obs_source_frame_destroy(dst);
	obs_source_frame_destroy(src);
}

static void every_format_equal_line_sizes(void **state)
{
	(void)state;
	for (size_t f = 0; f < FORMAT_COUNT; f++)
		for (size_t s = 0; s < SIZE_COUNT; s++)
			check_equal_line_sizes(&formats[f], sizes[s][0], sizes[s][1]);
}

/* Each plane's destination line size is the source's plus delta[plane]
 * (never 0). */
static void check_unequal_line_sizes(const struct format_case *fc, uint32_t width, uint32_t height, const int *delta)
{
	struct obs_source_frame *src = obs_source_frame_create(fc->format, width, height);
	struct obs_source_frame dst;
	uint32_t linesize[PLANES] = {0};
	assert_non_null(src);
	fill_source(src, fc);
	for (size_t p = 0; p < fc->planes; p++) {
		int64_t line = (int64_t)src->linesize[p] + delta[p];
		linesize[p] = line < 1 ? 1 : (uint32_t)line;
	}
	build_destination(&dst, fc, width, height, linesize);

	obs_source_frame_copy(&dst, src);
	check_planes(&dst, src, fc);
	for (size_t p = 0; p < fc->planes; p++)
		assert_int_equal(dst.linesize[p], linesize[p]);

	free_destination(&dst);
	obs_source_frame_destroy(src);
}

static void every_format_unequal_line_sizes(void **state)
{
	(void)state;
	/* Longer, shorter, mixed per plane, and one plane equal. */
	const int deltas[][PLANES] = {
		{16, 16, 16, 16}, {-1, -1, -1, -1}, {5, -1, 3, -1}, {-3, 0, 7, 0}, {0, 9, -1, 2},
	};
	for (size_t f = 0; f < FORMAT_COUNT; f++)
		for (size_t s = 0; s < SIZE_COUNT; s++)
			for (size_t d = 0; d < sizeof(deltas) / sizeof(deltas[0]); d++)
				check_unequal_line_sizes(&formats[f], sizes[s][0], sizes[s][1], deltas[d]);
}

static const uint32_t matrix_bits[16] = {
	0x3f800000, 0x00000000, 0x80000000, 0x7f800000, 0xff800000, 0x7fc00001, 0x7f800001, 0x00000001,
	0x3eaaaaab, 0xbf000000, 0x41200000, 0x12345678, 0xffffffff, 0x7f7fffff, 0x00800000, 0xc2c80000,
};
static const uint32_t range_min_bits[3] = {0x3d808081, 0x7fc00002, 0x80000000};
static const uint32_t range_max_bits[3] = {0x3f6b6b6c, 0x3f700000, 0x7f800001};

static void set_source_metadata(struct obs_source_frame *src, bool full_range)
{
	src->flip = true;
	src->flags = 0xa5;
	src->trc = 0x3c;
	src->full_range = full_range;
	src->max_luminance = 0xbeef;
	src->timestamp = 0x0123456789abcdefULL;
	memcpy(src->color_matrix, matrix_bits, sizeof(matrix_bits));
	memcpy(src->color_range_min, range_min_bits, sizeof(range_min_bits));
	memcpy(src->color_range_max, range_max_bits, sizeof(range_max_bits));
	src->refs = 3;
	src->prev_frame = false;
}

static void set_destination_metadata(struct obs_source_frame *dst, bool full_range)
{
	dst->flip = false;
	dst->flags = 0x11;
	dst->trc = 0x01;
	dst->full_range = full_range;
	dst->max_luminance = 7;
	dst->timestamp = 42;
	memset(dst->color_matrix, 0x22, sizeof(dst->color_matrix));
	memset(dst->color_range_min, 0x33, sizeof(dst->color_range_min));
	memset(dst->color_range_max, 0x44, sizeof(dst->color_range_max));
	dst->refs = 9;
	dst->prev_frame = true;
}

/* Copies flip, flags, trc, full_range, max_luminance, timestamp and the
 * colour matrix bit for bit; the colour range only when the destination is
 * not full range after full_range was copied, so it follows the source's
 * full_range, not the destination's. format, width, height, data, linesize,
 * refs and prev_frame stay the destination's. */
static void check_metadata(bool src_full, bool dst_full)
{
	struct obs_source_frame *src = obs_source_frame_create(VIDEO_FORMAT_NV12, 6, 4);
	struct obs_source_frame *dst = obs_source_frame_create(VIDEO_FORMAT_NV12, 6, 4);
	assert_non_null(src);
	assert_non_null(dst);
	set_source_metadata(src, src_full);
	set_destination_metadata(dst, dst_full);
	struct obs_source_frame before = *dst;

	obs_source_frame_copy(dst, src);

	assert_true(dst->flip);
	assert_int_equal(dst->flags, 0xa5);
	assert_int_equal(dst->trc, 0x3c);
	assert_int_equal(dst->full_range, src_full);
	assert_int_equal(dst->max_luminance, 0xbeef);
	assert_true(dst->timestamp == 0x0123456789abcdefULL);
	assert_memory_equal(dst->color_matrix, matrix_bits, sizeof(matrix_bits));
	if (src_full) {
		assert_memory_equal(dst->color_range_min, before.color_range_min, sizeof(before.color_range_min));
		assert_memory_equal(dst->color_range_max, before.color_range_max, sizeof(before.color_range_max));
	} else {
		assert_memory_equal(dst->color_range_min, range_min_bits, sizeof(range_min_bits));
		assert_memory_equal(dst->color_range_max, range_max_bits, sizeof(range_max_bits));
	}
	assert_int_equal(dst->format, before.format);
	assert_int_equal(dst->width, before.width);
	assert_int_equal(dst->height, before.height);
	assert_memory_equal(dst->data, before.data, sizeof(before.data));
	assert_memory_equal(dst->linesize, before.linesize, sizeof(before.linesize));
	assert_int_equal(dst->refs, 9);
	assert_true(dst->prev_frame);

	/* The source is only read. */
	assert_int_equal(src->full_range, src_full);
	assert_int_equal(src->refs, 3);
	assert_false(src->prev_frame);
	assert_true(src->timestamp == 0x0123456789abcdefULL);

	obs_source_frame_destroy(dst);
	obs_source_frame_destroy(src);
}

static void metadata_and_colour_range(void **state)
{
	(void)state;
	check_metadata(false, false);
	check_metadata(false, true);
	check_metadata(true, false);
	check_metadata(true, true);
}

/* The planes the metadata test leaves alone are still copied. */
static void metadata_copy_also_copies_planes(void **state)
{
	(void)state;
	const struct format_case *fc = &formats[1]; /* NV12 */
	struct obs_source_frame *src = obs_source_frame_create(fc->format, 6, 4);
	struct obs_source_frame *dst = obs_source_frame_create(fc->format, 6, 4);
	assert_non_null(src);
	assert_non_null(dst);
	fill_source(src, fc);
	set_source_metadata(src, false);
	for (size_t p = 0; p < fc->planes; p++)
		memset(dst->data[p], FILL_DST, (size_t)dst->linesize[p] * plane_rows(fc, p, 4));

	obs_source_frame_copy(dst, src);
	check_planes(dst, src, fc);
	assert_memory_equal(dst->color_range_min, range_min_bits, sizeof(range_min_bits));

	obs_source_frame_destroy(dst);
	obs_source_frame_destroy(src);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(every_format_equal_line_sizes),
		cmocka_unit_test(every_format_unequal_line_sizes),
		cmocka_unit_test(metadata_and_colour_range),
		cmocka_unit_test(metadata_copy_also_copies_planes),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
