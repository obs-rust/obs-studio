/* Characterize the public obs_source_frame_copy against the actual library
 * before its extraction and Rust replacement (#5720). Frames are caller-owned
 * buffers: no OBS core, video device, or graphics context is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>
#include "obs.h"

#define SRC_FILL 0x11
#define DST_FILL 0xa5

struct plane_spec {
	uint32_t src_linesize;
	uint32_t dst_linesize;
	uint32_t rows; /* buffer rows to allocate for both frames */
};

struct frames {
	struct obs_source_frame src;
	struct obs_source_frame dst;
	size_t sizes[MAX_AV_PLANES][2];
};

/* Source plane bytes are a per-plane pattern; destination bytes are DST_FILL,
 * so untouched destination bytes stay recognizable. */
static void frames_init(struct frames *f, enum video_format src_format, uint32_t src_height,
			enum video_format dst_format, uint32_t dst_height, const struct plane_spec *planes,
			size_t count)
{
	memset(f, 0, sizeof(*f));
	memset(&f->dst, 0x5a, sizeof(f->dst));
	f->dst.full_range = true;
	f->dst.flip = false;
	f->dst.prev_frame = false;
	f->dst.format = dst_format;
	f->dst.width = 77;
	f->dst.height = dst_height;
	f->dst.refs = 3;
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		f->dst.data[i] = NULL;
		f->dst.linesize[i] = 0;
	}

	f->src.format = src_format;
	f->src.width = 99;
	f->src.height = src_height;
	f->src.timestamp = 0x0102030405060708ULL;
	f->src.full_range = false;
	f->src.max_luminance = 1000;
	f->src.flip = true;
	f->src.flags = OBS_SOURCE_FRAME_LINEAR_ALPHA;
	f->src.trc = VIDEO_TRC_PQ;
	f->src.refs = 9;
	for (size_t i = 0; i < 16; i++)
		f->src.color_matrix[i] = (float)i + 0.5f;
	for (size_t i = 0; i < 3; i++) {
		f->src.color_range_min[i] = 0.0625f * (float)(i + 1);
		f->src.color_range_max[i] = 0.9f + 0.01f * (float)i;
		f->dst.color_range_min[i] = -1.0f;
		f->dst.color_range_max[i] = -2.0f;
	}

	for (size_t i = 0; i < count; i++) {
		size_t src_size = (size_t)planes[i].src_linesize * planes[i].rows;
		size_t dst_size = (size_t)planes[i].dst_linesize * planes[i].rows;
		f->sizes[i][0] = src_size;
		f->sizes[i][1] = dst_size;
		f->src.linesize[i] = planes[i].src_linesize;
		f->dst.linesize[i] = planes[i].dst_linesize;
		f->src.data[i] = malloc(src_size ? src_size : 1);
		f->dst.data[i] = malloc(dst_size ? dst_size : 1);
		assert_non_null(f->src.data[i]);
		assert_non_null(f->dst.data[i]);
		for (size_t b = 0; b < src_size; b++)
			f->src.data[i][b] = (uint8_t)(SRC_FILL * (i + 1) + b);
		memset(f->dst.data[i], DST_FILL, dst_size);
	}
}

static void frames_free(struct frames *f)
{
	for (size_t i = 0; i < MAX_AV_PLANES; i++) {
		free(f->src.data[i]);
		free(f->dst.data[i]);
	}
}

/* The destination after a copy: the metadata of the source, its own size,
 * format, strides, plane pointers, refs and prev_frame. Color range follows
 * the copied full_range flag. */
static void expect_metadata(const struct obs_source_frame *before, const struct obs_source_frame *src,
			    const struct obs_source_frame *after)
{
	struct obs_source_frame expected;
	memcpy(&expected, before, sizeof(expected));
	expected.flip = src->flip;
	expected.flags = src->flags;
	expected.trc = src->trc;
	expected.full_range = src->full_range;
	expected.max_luminance = src->max_luminance;
	expected.timestamp = src->timestamp;
	memcpy(expected.color_matrix, src->color_matrix, sizeof(expected.color_matrix));
	if (!src->full_range) {
		memcpy(expected.color_range_min, src->color_range_min, sizeof(expected.color_range_min));
		memcpy(expected.color_range_max, src->color_range_max, sizeof(expected.color_range_max));
	}
	assert_memory_equal(after, &expected, sizeof(expected));
}

/* Checks one destination plane: `rows` rows of `bytes` copied at each stride,
 * everything else still DST_FILL. */
static void expect_plane(const struct frames *f, size_t plane, uint32_t rows, size_t bytes)
{
	const uint32_t src_stride = f->src.linesize[plane];
	const uint32_t dst_stride = f->dst.linesize[plane];
	const size_t dst_size = f->sizes[plane][1];
	const uint8_t *dst = f->dst.data[plane];
	const uint8_t *src = f->src.data[plane];
	for (size_t b = 0; b < dst_size; b++) {
		size_t row = b / dst_stride;
		size_t col = b % dst_stride;
		if (row < rows && col < bytes)
			assert_int_equal(dst[b], src[row * src_stride + col]);
		else
			assert_int_equal(dst[b], DST_FILL);
	}
}

static void expect_plane_untouched(const struct frames *f, size_t plane)
{
	for (size_t b = 0; b < f->sizes[plane][1]; b++)
		assert_int_equal(f->dst.data[plane][b], DST_FILL);
}

static void copy_and_check_metadata(struct frames *f)
{
	struct obs_source_frame before;
	struct obs_source_frame src_before;
	memcpy(&before, &f->dst, sizeof(before));
	memcpy(&src_before, &f->src, sizeof(src_before));
	obs_source_frame_copy(&f->dst, &f->src);
	expect_metadata(&before, &f->src, &f->dst);
	assert_memory_equal(&f->src, &src_before, sizeof(src_before));
}

/* Packed RGBA with equal strides: one whole-plane copy of linesize * height;
 * limited range copies the color range. */
static void packed_equal_strides_copy_whole_plane(void **state)
{
	(void)state;
	struct frames f;
	const struct plane_spec planes[] = {{16, 16, 6}};
	frames_init(&f, VIDEO_FORMAT_RGBA, 6, VIDEO_FORMAT_RGBA, 4, planes, 1);
	copy_and_check_metadata(&f);
	/* Rows come from the destination height, not the source's. */
	expect_plane(&f, 0, 4, 16);
	frames_free(&f);
}

/* Full range: the destination color range is left as it was. */
static void full_range_keeps_destination_color_range(void **state)
{
	(void)state;
	struct frames f;
	const struct plane_spec planes[] = {{8, 8, 3}};
	frames_init(&f, VIDEO_FORMAT_BGRA, 3, VIDEO_FORMAT_BGRA, 3, planes, 1);
	f.src.full_range = true;
	copy_and_check_metadata(&f);
	assert_true(f.dst.full_range);
	for (size_t i = 0; i < 3; i++) {
		assert_true(f.dst.color_range_min[i] == -1.0f);
		assert_true(f.dst.color_range_max[i] == -2.0f);
	}
	expect_plane(&f, 0, 3, 8);
	frames_free(&f);
}

/* Planar I420 with padded strides in both directions: row copies of the
 * smaller line size, chroma rows rounded up from an odd height, padding
 * untouched. */
static void planar_padded_strides_copy_rows(void **state)
{
	(void)state;
	struct frames f;
	const struct plane_spec planes[] = {{12, 16, 5}, {6, 4, 3}, {8, 8, 3}};
	frames_init(&f, VIDEO_FORMAT_I420, 5, VIDEO_FORMAT_I420, 5, planes, 3);
	copy_and_check_metadata(&f);
	expect_plane(&f, 0, 5, 12);
	expect_plane(&f, 1, 3, 4);
	expect_plane(&f, 2, 3, 8);
	frames_free(&f);
}

/* The plan follows the source format: an NV12 source copies two planes even
 * when the destination says RGBA, and later destination planes stay as they
 * were. */
static void source_format_selects_planes(void **state)
{
	(void)state;
	struct frames f;
	const struct plane_spec planes[] = {{8, 8, 4}, {8, 10, 2}, {4, 4, 4}};
	frames_init(&f, VIDEO_FORMAT_NV12, 4, VIDEO_FORMAT_RGBA, 4, planes, 3);
	copy_and_check_metadata(&f);
	expect_plane(&f, 0, 4, 8);
	expect_plane(&f, 1, 2, 8);
	expect_plane_untouched(&f, 2);
	frames_free(&f);
}

/* Four full-height planes for YUVA and I40A's alpha plane at full height. */
static void alpha_planes(void **state)
{
	(void)state;
	struct frames f;
	const struct plane_spec yuva[] = {{4, 4, 3}, {4, 4, 3}, {4, 6, 3}, {4, 4, 3}};
	frames_init(&f, VIDEO_FORMAT_YUVA, 3, VIDEO_FORMAT_YUVA, 3, yuva, 4);
	copy_and_check_metadata(&f);
	expect_plane(&f, 0, 3, 4);
	expect_plane(&f, 1, 3, 4);
	expect_plane(&f, 2, 3, 4);
	expect_plane(&f, 3, 3, 4);
	frames_free(&f);

	const struct plane_spec i40a[] = {{4, 4, 3}, {2, 2, 2}, {2, 2, 2}, {4, 4, 3}};
	frames_init(&f, VIDEO_FORMAT_I40A, 3, VIDEO_FORMAT_I40A, 3, i40a, 4);
	copy_and_check_metadata(&f);
	expect_plane(&f, 0, 3, 4);
	expect_plane(&f, 1, 2, 2);
	expect_plane(&f, 2, 2, 2);
	expect_plane(&f, 3, 3, 4);
	frames_free(&f);
}

/* P216 has no copy plan: metadata is copied, no plane bytes are. */
static void unimplemented_format_copies_metadata_only(void **state)
{
	(void)state;
	struct frames f;
	const struct plane_spec planes[] = {{8, 8, 2}, {8, 8, 2}};
	frames_init(&f, VIDEO_FORMAT_P216, 2, VIDEO_FORMAT_P216, 2, planes, 2);
	copy_and_check_metadata(&f);
	expect_plane_untouched(&f, 0);
	expect_plane_untouched(&f, 1);
	frames_free(&f);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(packed_equal_strides_copy_whole_plane),
		cmocka_unit_test(full_range_keeps_destination_color_range),
		cmocka_unit_test(planar_padded_strides_copy_rows),
		cmocka_unit_test(source_format_selects_planes),
		cmocka_unit_test(alpha_planes),
		cmocka_unit_test(unimplemented_format_copies_metadata_only),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
