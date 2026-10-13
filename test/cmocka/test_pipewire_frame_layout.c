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
#include <spa/param/video/type-info.h>
#include <spa/debug/types.h>

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
 * display is needed.
 *
 * obsproject/obs-studio#13744 (reported by Owlock): a camera that offers only
 * I420 (DroidCam's v4l2loopback device, YU12) could not be used: I420 was not
 * in the format table, so the camera source listed no format for it and never
 * offered I420 to the camera. I420 has three planes, luma and two chroma
 * planes of half the width and half the rows, in one block or one block each.
 * YV12 is the same with the chroma planes swapped.
 *
 * A follow-up found in review of that change: at an odd width a chroma row
 * holds (w + 1) / 2 bytes, but the chroma stride taken from an odd luma
 * stride was rounded down (5 / 2 = 2 for a 5 pixel wide frame, whose chroma
 * rows need 3), so every tightly packed odd-width I420 or YV12 frame was
 * dropped and the source stayed black. */

#define MAX_BLOCKS 3

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

	if (!obs_pw_video_format_from_spa_format(spa_format, &format)) {
		fprintf(stderr, "%s is not a format the PipeWire camera source supports\n",
			spa_debug_type_find_short_name(spa_type_video_format, spa_format));
		fail();
	}
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

/* I420 in one block, as v4l2loopback and UVC cameras send it (#13744): luma,
 * then the U and V planes, each half the luma stride and half the rows. */
static void i420_one_block(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 64, h = 48, size = w * h * 3 / 2;

	UNUSED_PARAMETER(state);
	add_block(&tb, size, 0, size, w);

	expect_used("I420, one block", layout(&frame, &tb, SPA_VIDEO_FORMAT_I420, w, h));
	if (frame.format != VIDEO_FORMAT_I420) {
		fprintf(stderr, "I420, one block: the frame is %s, expected I420\n",
			get_video_format_name(frame.format));
		fail();
	}
	expect_plane("I420, one block", &frame, &tb, 0, 0, 0, w, h);
	expect_plane("I420, one block", &frame, &tb, 1, 0, (size_t)w * h, w / 2, h / 2);
	expect_plane("I420, one block", &frame, &tb, 2, 0, (size_t)w * h + (w / 2) * (h / 2), w / 2, h / 2);
	free_blocks(&tb);
}

/* Rows padded to 128 bytes (chroma rows to 64), odd width and height: the
 * chroma planes have (w + 1) / 2 bytes in (h + 1) / 2 rows. */
static void i420_one_block_padded_rows(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 99, h = 45, stride = 128, c_stride = 64, c_rows = (h + 1) / 2;
	const uint32_t size = stride * h + 2 * c_stride * c_rows;

	UNUSED_PARAMETER(state);
	add_block(&tb, size, 0, size, stride);

	expect_used("I420, padded rows", layout(&frame, &tb, SPA_VIDEO_FORMAT_I420, w, h));
	expect_plane("I420, padded rows", &frame, &tb, 0, 0, 0, stride, h);
	expect_plane("I420, padded rows", &frame, &tb, 1, 0, (size_t)stride * h, c_stride, c_rows);
	expect_plane("I420, padded rows", &frame, &tb, 2, 0, (size_t)stride * h + c_stride * c_rows, c_stride, c_rows);
	free_blocks(&tb);
}

/* No chunk stride: luma rows are the width rounded up to 4 bytes, chroma
 * rows half that. */
static void i420_one_block_without_stride(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 62, h = 48, stride = 64, size = stride * h * 3 / 2;

	UNUSED_PARAMETER(state);
	add_block(&tb, size, 0, size, 0);

	expect_used("I420, no chunk stride", layout(&frame, &tb, SPA_VIDEO_FORMAT_I420, w, h));
	expect_plane("I420, no chunk stride", &frame, &tb, 0, 0, 0, stride, h);
	expect_plane("I420, no chunk stride", &frame, &tb, 1, 0, (size_t)stride * h, stride / 2, h / 2);
	expect_plane("I420, no chunk stride", &frame, &tb, 2, 0, (size_t)stride * h + (stride / 2) * (h / 2),
		     stride / 2, h / 2);
	free_blocks(&tb);
}

/* One block per plane, each with its own chunk offset and stride. */
static void i420_three_blocks(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 64, h = 48, y_stride = 64, u_offset = 16, u_stride = 48, v_offset = 32, v_stride = 40;

	UNUSED_PARAMETER(state);
	add_block(&tb, y_stride * h, 0, y_stride * h, y_stride);
	add_block(&tb, u_offset + u_stride * h / 2, u_offset, u_stride * h / 2, u_stride);
	add_block(&tb, v_offset + v_stride * h / 2, v_offset, v_stride * h / 2, v_stride);

	expect_used("I420, three blocks", layout(&frame, &tb, SPA_VIDEO_FORMAT_I420, w, h));
	expect_plane("I420, three blocks", &frame, &tb, 0, 0, 0, y_stride, h);
	expect_plane("I420, three blocks", &frame, &tb, 1, 1, u_offset, u_stride, h / 2);
	expect_plane("I420, three blocks", &frame, &tb, 2, 2, v_offset, v_stride, h / 2);
	free_blocks(&tb);
}

/* A chunk one chroma row short of a whole frame, or a buffer with two of the
 * three planes, must be dropped. */
static void i420_chunk_too_small_is_dropped(void **state)
{
	struct test_buffer short_chunk = {0}, two_blocks = {0};
	struct obs_source_frame frame;
	const uint32_t w = 64, h = 48, size = w * h * 3 / 2;

	UNUSED_PARAMETER(state);
	add_block(&short_chunk, size, 0, size - w / 2, w);
	expect_dropped("I420, chunk a row short", layout(&frame, &short_chunk, SPA_VIDEO_FORMAT_I420, w, h));

	add_block(&two_blocks, w * h, 0, w * h, w);
	add_block(&two_blocks, w * h / 2, 0, w * h / 2, w / 2);
	expect_dropped("I420, two of three planes", layout(&frame, &two_blocks, SPA_VIDEO_FORMAT_I420, w, h));
	free_blocks(&short_chunk);
	free_blocks(&two_blocks);
}

/* YV12 is I420 with V before U: the frame's U plane is the third plane in
 * memory. */
static void yv12_chroma_planes_swapped(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 64, h = 48, size = w * h * 3 / 2;

	UNUSED_PARAMETER(state);
	add_block(&tb, size, 0, size, w);

	expect_used("YV12, one block", layout(&frame, &tb, SPA_VIDEO_FORMAT_YV12, w, h));
	if (frame.format != VIDEO_FORMAT_I420) {
		fprintf(stderr, "YV12, one block: the frame is %s, expected I420\n",
			get_video_format_name(frame.format));
		fail();
	}
	expect_plane("YV12, one block", &frame, &tb, 0, 0, 0, w, h);
	expect_plane("YV12, one block", &frame, &tb, 1, 0, (size_t)w * h + (w / 2) * (h / 2), w / 2, h / 2);
	expect_plane("YV12, one block", &frame, &tb, 2, 0, (size_t)w * h, w / 2, h / 2);
	free_blocks(&tb);
}

/* Odd width, rows not padded: luma rows of w bytes, chroma rows of
 * (w + 1) / 2 bytes in (h + 1) / 2 rows, all in one block, as V4L2 lays
 * out YU12 and YV12 (DIV_ROUND_UP of the width for the chroma planes). */
static void expect_odd_width_tight(uint32_t spa_format, uint32_t w, uint32_t h)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const bool yv12 = spa_format == SPA_VIDEO_FORMAT_YV12;
	const uint32_t c_stride = (w + 1) / 2, c_rows = (h + 1) / 2;
	const uint32_t first = w * h, second = first + c_stride * c_rows, size = second + c_stride * c_rows;
	char name[64];

	snprintf(name, sizeof(name), "%s, %ux%u, tightly packed", yv12 ? "YV12" : "I420", w, h);
	add_block(&tb, size, 0, size, (int32_t)w);

	expect_used(name, layout(&frame, &tb, spa_format, w, h));
	expect_plane(name, &frame, &tb, 0, 0, 0, w, h);
	expect_plane(name, &frame, &tb, 1, 0, yv12 ? second : first, c_stride, c_rows);
	expect_plane(name, &frame, &tb, 2, 0, yv12 ? first : second, c_stride, c_rows);
	free_blocks(&tb);
}

static void i420_odd_width_tightly_packed(void **state)
{
	UNUSED_PARAMETER(state);
	expect_odd_width_tight(SPA_VIDEO_FORMAT_I420, 5, 3);
	expect_odd_width_tight(SPA_VIDEO_FORMAT_I420, 641, 481);
}

static void yv12_odd_width_tightly_packed(void **state)
{
	UNUSED_PARAMETER(state);
	expect_odd_width_tight(SPA_VIDEO_FORMAT_YV12, 5, 3);
	expect_odd_width_tight(SPA_VIDEO_FORMAT_YV12, 641, 481);
}

/* Odd width, luma rows padded by an even number of bytes: the chroma rows
 * have half as many pad bytes (V4L2's rule for YU12), so a chroma row is
 * half the luma stride rounded up: 7 -> 4 for a width of 5. */
static void i420_odd_width_odd_stride(void **state)
{
	struct test_buffer tb = {0};
	struct obs_source_frame frame;
	const uint32_t w = 5, h = 3, stride = 7, c_stride = 4, c_rows = 2;
	const uint32_t size = stride * h + 2 * c_stride * c_rows;

	UNUSED_PARAMETER(state);
	add_block(&tb, size, 0, size, stride);

	expect_used("I420, odd width, luma stride 7", layout(&frame, &tb, SPA_VIDEO_FORMAT_I420, w, h));
	expect_plane("I420, odd width, luma stride 7", &frame, &tb, 0, 0, 0, stride, h);
	expect_plane("I420, odd width, luma stride 7", &frame, &tb, 1, 0, (size_t)stride * h, c_stride, c_rows);
	expect_plane("I420, odd width, luma stride 7", &frame, &tb, 2, 0, (size_t)stride * h + c_stride * c_rows,
		     c_stride, c_rows);
	free_blocks(&tb);
}

/* Odd width, one block per plane: a chroma block without a stride of its
 * own gets half the luma stride rounded up; one with a stride keeps it. */
static void i420_odd_width_three_blocks(void **state)
{
	struct test_buffer no_stride = {0}, own_stride = {0};
	struct obs_source_frame frame;
	const uint32_t w = 5, h = 3, c_stride = 3, c_rows = 2;

	UNUSED_PARAMETER(state);
	add_block(&no_stride, w * h, 0, w * h, w);
	add_block(&no_stride, c_stride * c_rows, 0, c_stride * c_rows, 0);
	add_block(&no_stride, c_stride * c_rows, 0, c_stride * c_rows, 0);
	expect_used("I420, odd width, three blocks, no chroma stride",
		    layout(&frame, &no_stride, SPA_VIDEO_FORMAT_I420, w, h));
	expect_plane("I420, odd width, three blocks, no chroma stride", &frame, &no_stride, 0, 0, 0, w, h);
	expect_plane("I420, odd width, three blocks, no chroma stride", &frame, &no_stride, 1, 1, 0, c_stride, c_rows);
	expect_plane("I420, odd width, three blocks, no chroma stride", &frame, &no_stride, 2, 2, 0, c_stride, c_rows);

	add_block(&own_stride, w * h, 0, w * h, w);
	add_block(&own_stride, c_stride * c_rows, 0, c_stride * c_rows, c_stride);
	add_block(&own_stride, c_stride * c_rows, 0, c_stride * c_rows, c_stride);
	expect_used("I420, odd width, three blocks, chroma stride 3",
		    layout(&frame, &own_stride, SPA_VIDEO_FORMAT_I420, w, h));
	expect_plane("I420, odd width, three blocks, chroma stride 3", &frame, &own_stride, 1, 1, 0, c_stride, c_rows);
	expect_plane("I420, odd width, three blocks, chroma stride 3", &frame, &own_stride, 2, 2, 0, c_stride, c_rows);

	free_blocks(&no_stride);
	free_blocks(&own_stride);
}

/* Guards for the rounding: padded even strides keep their halves, and a
 * buffer too small for the rounded-up chroma rows is still dropped, as is a
 * luma stride shorter than a row. Nothing may be read past the chunk. */
static void i420_odd_width_guards(void **state)
{
	struct test_buffer padded = {0}, one_byte_short = {0}, floor_sized = {0}, narrow = {0},
			   short_chroma_block = {0};
	struct obs_source_frame frame;
	const uint32_t w = 5, h = 3, tight = 15 + 2 * 3 * 2;

	UNUSED_PARAMETER(state);

	/* GStreamer-style padding: luma rows of 8, chroma rows of 4. */
	add_block(&padded, 8 * h + 2 * 4 * 2, 0, 8 * h + 2 * 4 * 2, 8);
	expect_used("I420, odd width, padded to 8", layout(&frame, &padded, SPA_VIDEO_FORMAT_I420, w, h));
	expect_plane("I420, odd width, padded to 8", &frame, &padded, 0, 0, 0, 8, h);
	expect_plane("I420, odd width, padded to 8", &frame, &padded, 1, 0, 8 * h, 4, 2);
	expect_plane("I420, odd width, padded to 8", &frame, &padded, 2, 0, 8 * h + 4 * 2, 4, 2);

	add_block(&one_byte_short, tight, 0, tight - 1, w);
	expect_dropped("I420, odd width, chunk one byte short",
		       layout(&frame, &one_byte_short, SPA_VIDEO_FORMAT_I420, w, h));
	expect_dropped("YV12, odd width, chunk one byte short",
		       layout(&frame, &one_byte_short, SPA_VIDEO_FORMAT_YV12, w, h));

	/* Only big enough for chroma rows of 2 bytes (the old rounding). */
	add_block(&floor_sized, 15 + 2 * 2 * 2, 0, 15 + 2 * 2 * 2, w);
	expect_dropped("I420, odd width, chroma rows of 2", layout(&frame, &floor_sized, SPA_VIDEO_FORMAT_I420, w, h));

	add_block(&narrow, tight, 0, tight, w - 1);
	expect_dropped("I420, luma stride shorter than a row", layout(&frame, &narrow, SPA_VIDEO_FORMAT_I420, w, h));

	add_block(&short_chroma_block, w * h, 0, w * h, w);
	add_block(&short_chroma_block, 3 * 2, 0, 3 * 2, 0);
	add_block(&short_chroma_block, 3 * 2 - 1, 0, 3 * 2 - 1, 0);
	expect_dropped("I420, odd width, third block one byte short",
		       layout(&frame, &short_chroma_block, SPA_VIDEO_FORMAT_I420, w, h));

	free_blocks(&padded);
	free_blocks(&one_byte_short);
	free_blocks(&floor_sized);
	free_blocks(&narrow);
	free_blocks(&short_chroma_block);
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
		cmocka_unit_test(i420_one_block),
		cmocka_unit_test(i420_one_block_padded_rows),
		cmocka_unit_test(i420_one_block_without_stride),
		cmocka_unit_test(i420_three_blocks),
		cmocka_unit_test(i420_chunk_too_small_is_dropped),
		cmocka_unit_test(yv12_chroma_planes_swapped),
		cmocka_unit_test(i420_odd_width_tightly_packed),
		cmocka_unit_test(yv12_odd_width_tightly_packed),
		cmocka_unit_test(i420_odd_width_odd_stride),
		cmocka_unit_test(i420_odd_width_three_blocks),
		cmocka_unit_test(i420_odd_width_guards),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
