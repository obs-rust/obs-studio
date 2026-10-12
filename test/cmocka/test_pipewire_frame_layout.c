#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <spa/buffer/buffer.h>
#include <spa/param/video/raw.h>

#include "formats.h"
#include "frame-layout.h"

/* obsproject/obs-studio#13823 (reported by EberhartLeberhart) and
 * obsproject/obs-studio#14002 (theCarlG): a PipeWire camera that sends NV12
 * shows a green, banded picture or crashes OBS as soon as it streams.
 *
 * The camera source hands libobs one data pointer per PipeWire data block and
 * a single line size of width * 2. NV12 has two planes: a luma plane of width
 * bytes per row and an interleaved chroma plane of half the rows. Cameras
 * usually send both planes in one block, so the chroma plane had no pointer
 * and no line size, and libobs read twice the luma plane, past the end of
 * the buffer. The stride and offset the producer gives in the buffer's chunk
 * were ignored for every format.
 *
 * These cases build PipeWire buffers the way producers send them (one block,
 * one block with padded rows, one block per plane, an offset into the block)
 * and check the planes the frame gets: every plane starts where the producer
 * put it, has the producer's stride, and lies inside the buffer. Buffers
 * that cannot hold a whole frame must be dropped. No PipeWire daemon or
 * display is needed. */

#define MAX_BLOCKS 2

struct test_buffer {
	struct spa_buffer buffer;
	struct spa_data datas[MAX_BLOCKS];
	struct spa_chunk chunks[MAX_BLOCKS];
	uint8_t *memory[MAX_BLOCKS];
};

/* Fills in one block of maxsize bytes; its chunk is set by the caller. */
static void add_block(struct test_buffer *tb, uint32_t maxsize, uint32_t offset, uint32_t size, int32_t stride)
{
	uint32_t i = tb->buffer.n_datas++;

	tb->memory[i] = test_calloc(1, maxsize);
	tb->datas[i] = (struct spa_data){
		.type = SPA_DATA_MemPtr,
		.maxsize = maxsize,
		.data = tb->memory[i],
		.chunk = &tb->chunks[i],
	};
	tb->chunks[i] = (struct spa_chunk){.offset = offset, .size = size, .stride = stride};
	tb->buffer.datas = tb->datas;
}

static void free_blocks(struct test_buffer *tb)
{
	for (uint32_t i = 0; i < tb->buffer.n_datas; i++)
		test_free(tb->memory[i]);
}

static struct obs_pw_video_format format_of(uint32_t spa_format)
{
	struct obs_pw_video_format format = {0};

	assert_true(obs_pw_video_format_from_spa_format(spa_format, &format));
	return format;
}

static bool layout(struct obs_source_frame *frame, const struct test_buffer *tb, uint32_t spa_format, uint32_t width,
		   uint32_t height)
{
	struct obs_pw_video_format format = format_of(spa_format);

	memset(frame, 0, sizeof(*frame));
	frame->width = width;
	frame->height = height;
	frame->format = format.video_format;
	return obs_pw_frame_layout(frame, &tb->buffer, &format);
}

/* Plane p must start at byte `offset` of block `block`, have line size
 * `stride`, and its `rows` rows must lie inside the block's memory. */
static void expect_plane(const char *name, const struct obs_source_frame *frame, const struct test_buffer *tb,
			 uint32_t p, uint32_t block, size_t offset, uint32_t stride, uint32_t rows)
{
	const uint8_t *start = tb->memory[block];
	const uint8_t *end = start + tb->datas[block].maxsize;

	if (frame->data[p] != start + offset || frame->linesize[p] != stride) {
		fprintf(stderr,
			"%s: plane %u is at block %u + %td with line size %u, expected block %u + %zu with line size %u\n",
			name, p, block, frame->data[p] ? frame->data[p] - start : -1, frame->linesize[p], block, offset,
			stride);
		fail();
	}
	if (frame->data[p] + (size_t)frame->linesize[p] * rows > end) {
		fprintf(stderr, "%s: plane %u (%u rows of %u bytes) ends past the buffer\n", name, p, rows,
			frame->linesize[p]);
		fail();
	}
}

static void expect_dropped(const char *name, bool used)
{
	if (used) {
		fprintf(stderr, "%s: a buffer that cannot hold the frame was used instead of dropped\n", name);
		fail();
	}
}

static void expect_used(const char *name, bool used)
{
	if (!used) {
		fprintf(stderr, "%s: a valid buffer was dropped\n", name);
		fail();
	}
}

/* The common case (#13823, #14002): both planes in one block, rows not
 * padded, as UVC cameras through the V4L2 SPA plugin and libcamera send. */
static void nv12_one_block(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 64, h = 48, size = w * h * 3 / 2;

	UNUSED_PARAMETER(state);
	add_block(&tb, size, 0, size, w);

	expect_used("NV12, one block", layout(&frame, &tb, SPA_VIDEO_FORMAT_NV12, w, h));
	expect_plane("NV12, one block", &frame, &tb, 0, 0, 0, w, h);
	expect_plane("NV12, one block", &frame, &tb, 1, 0, (size_t)w * h, w, h / 2);
	free_blocks(&tb);
}

/* Rows padded to 128 bytes, odd height (the chroma plane has (h + 1) / 2
 * rows). */
static void nv12_one_block_padded_rows(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 100, h = 45, stride = 128, size = stride * (h + (h + 1) / 2);

	UNUSED_PARAMETER(state);
	add_block(&tb, size, 0, size, stride);

	expect_used("NV12, padded rows", layout(&frame, &tb, SPA_VIDEO_FORMAT_NV12, w, h));
	expect_plane("NV12, padded rows", &frame, &tb, 0, 0, 0, stride, h);
	expect_plane("NV12, padded rows", &frame, &tb, 1, 0, (size_t)stride * h, stride, (h + 1) / 2);
	free_blocks(&tb);
}

/* A producer that leaves the chunk stride at 0: rows are taken to be the
 * width rounded up to 4 bytes, as for the packed formats. */
static void nv12_one_block_without_stride(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 62, h = 48, stride = 64, size = stride * h * 3 / 2;

	UNUSED_PARAMETER(state);
	add_block(&tb, size, 0, size, 0);

	expect_used("NV12, no chunk stride", layout(&frame, &tb, SPA_VIDEO_FORMAT_NV12, w, h));
	expect_plane("NV12, no chunk stride", &frame, &tb, 0, 0, 0, stride, h);
	expect_plane("NV12, no chunk stride", &frame, &tb, 1, 0, (size_t)stride * h, stride, h / 2);
	free_blocks(&tb);
}

/* One block per plane, each with its own chunk offset and stride. */
static void nv12_two_blocks(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 64, h = 48, y_stride = 64, uv_offset = 32, uv_stride = 80;

	UNUSED_PARAMETER(state);
	add_block(&tb, y_stride * h, 0, y_stride * h, y_stride);
	add_block(&tb, uv_offset + uv_stride * h / 2, uv_offset, uv_stride * h / 2, uv_stride);

	expect_used("NV12, two blocks", layout(&frame, &tb, SPA_VIDEO_FORMAT_NV12, w, h));
	expect_plane("NV12, two blocks", &frame, &tb, 0, 0, 0, y_stride, h);
	expect_plane("NV12, two blocks", &frame, &tb, 1, 1, uv_offset, uv_stride, h / 2);
	free_blocks(&tb);
}

/* A block that holds only the luma plane cannot be shown. */
static void nv12_without_chroma_is_dropped(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 64, h = 48;

	UNUSED_PARAMETER(state);
	add_block(&tb, w * h, 0, w * h, w);

	expect_dropped("NV12, luma only", layout(&frame, &tb, SPA_VIDEO_FORMAT_NV12, w, h));
	free_blocks(&tb);
}

/* The chunk is written by the producer: one that points outside the mapped
 * block, or a stride smaller than a row, must not be read. */
static void chunk_outside_the_block_is_dropped(void **state)
{
	struct test_buffer tb = {0};
	struct test_buffer narrow = {0};
	struct obs_source_frame frame;
	const uint32_t w = 64, h = 48, size = w * h * 3 / 2;

	UNUSED_PARAMETER(state);
	add_block(&tb, size, 64, size, w);
	expect_dropped("NV12, chunk past the block", layout(&frame, &tb, SPA_VIDEO_FORMAT_NV12, w, h));

	add_block(&narrow, w * 2 * h, 0, w * 2 * h, w);
	expect_dropped("YUY2, stride shorter than a row", layout(&frame, &narrow, SPA_VIDEO_FORMAT_YUY2, w, h));
	free_blocks(&tb);
	free_blocks(&narrow);
}

/* Packed formats too: padded rows and an offset into the block. */
static void yuy2_chunk_stride_and_offset(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 30, h = 20, offset = 64, stride = 64;

	UNUSED_PARAMETER(state);
	add_block(&tb, offset + stride * h, offset, stride * h, stride);

	expect_used("YUY2, padded rows at an offset", layout(&frame, &tb, SPA_VIDEO_FORMAT_YUY2, w, h));
	expect_plane("YUY2, padded rows at an offset", &frame, &tb, 0, 0, offset, stride, h);
	free_blocks(&tb);
}

/* Guards: tightly packed YUY2 and RGBA, with and without a chunk stride,
 * worked before and must keep working. */
static void packed_formats_unchanged(void **state)
{
	struct test_buffer yuy2 = {0}, yuy2_no_stride = {0}, rgba = {0};
	struct obs_source_frame frame;
	const uint32_t w = 64, h = 48;

	UNUSED_PARAMETER(state);
	add_block(&yuy2, w * 2 * h, 0, w * 2 * h, w * 2);
	add_block(&yuy2_no_stride, w * 2 * h, 0, w * 2 * h, 0);
	add_block(&rgba, w * 4 * h, 0, w * 4 * h, w * 4);

	expect_used("YUY2", layout(&frame, &yuy2, SPA_VIDEO_FORMAT_YUY2, w, h));
	expect_plane("YUY2", &frame, &yuy2, 0, 0, 0, w * 2, h);
	expect_used("YUY2, no chunk stride", layout(&frame, &yuy2_no_stride, SPA_VIDEO_FORMAT_YUY2, w, h));
	expect_plane("YUY2, no chunk stride", &frame, &yuy2_no_stride, 0, 0, 0, w * 2, h);
	expect_used("RGBA", layout(&frame, &rgba, SPA_VIDEO_FORMAT_RGBA, w, h));
	expect_plane("RGBA", &frame, &rgba, 0, 0, 0, w * 4, h);

	free_blocks(&yuy2);
	free_blocks(&yuy2_no_stride);
	free_blocks(&rgba);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(nv12_one_block),
		cmocka_unit_test(nv12_one_block_padded_rows),
		cmocka_unit_test(nv12_one_block_without_stride),
		cmocka_unit_test(nv12_two_blocks),
		cmocka_unit_test(nv12_without_chroma_is_dropped),
		cmocka_unit_test(chunk_outside_the_block_is_dropped),
		cmocka_unit_test(yuy2_chunk_stride_and_offset),
		cmocka_unit_test(packed_formats_unchanged),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
