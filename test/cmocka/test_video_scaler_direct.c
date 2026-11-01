/* video_scaler_scale() (libobs/media-io/video-scaler-ffmpeg.c) ran sws_scale
 * into its own buffer and then copied every plane into the caller's frame
 * (#3192). It now writes straight into a frame whose planes have the internal
 * buffer's layout (32-byte aligned, rows at least as long and a multiple of
 * 32), and keeps the copy for any other layout.
 *
 * Each case scales the same picture into a frame with that layout and into
 * one with rows 16 bytes longer, which takes the copy, and requires the same
 * visible bytes from both. It prints the median time per frame of each, for
 * the PR's before/after numbers. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <cmocka.h>

#include <media-io/video-scaler.h>
#include <util/bmem.h>
#include <util/platform.h>

#define RUNS 15

struct frame {
	uint8_t *data[2];
	uint32_t linesize[2];
};

static void frame_alloc(struct frame *f, uint32_t width, uint32_t height, uint32_t extra)
{
	const uint32_t linesize = ((width + 31) & ~31u) + extra;
	for (int p = 0; p < 2; p++) {
		const uint32_t rows = p == 0 ? height : height / 2;
		f->linesize[p] = linesize;
		f->data[p] = bmalloc((size_t)linesize * rows + 64);
		memset(f->data[p], 0xa5, (size_t)linesize * rows + 64);
	}
}

static void frame_free(struct frame *f)
{
	bfree(f->data[0]);
	bfree(f->data[1]);
}

static int cmp_u64(const void *a, const void *b)
{
	uint64_t x = *(const uint64_t *)a, y = *(const uint64_t *)b;
	return x < y ? -1 : x > y;
}

static uint64_t scale_median(video_scaler_t *scaler, struct frame *out, const uint8_t *const in[],
			     const uint32_t in_linesize[])
{
	uint64_t t[RUNS];
	for (int i = 0; i < RUNS; i++) {
		uint64_t start = os_gettime_ns();
		assert_true(video_scaler_scale(scaler, out->data, out->linesize, in, in_linesize));
		t[i] = os_gettime_ns() - start;
	}
	qsort(t, RUNS, sizeof(t[0]), cmp_u64);
	return t[RUNS / 2];
}

static void run(uint32_t src_w, uint32_t src_h, uint32_t dst_w, uint32_t dst_h, const char *label)
{
	const struct video_scale_info src = {VIDEO_FORMAT_BGRA, src_w, src_h, VIDEO_RANGE_PARTIAL, VIDEO_CS_709};
	const struct video_scale_info dst = {VIDEO_FORMAT_NV12, dst_w, dst_h, VIDEO_RANGE_PARTIAL, VIDEO_CS_709};
	video_scaler_t *scaler = NULL;
	assert_int_equal(video_scaler_create(&scaler, &dst, &src, VIDEO_SCALE_BICUBIC), VIDEO_SCALER_SUCCESS);

	const uint32_t in_linesize[1] = {src_w * 4};
	uint8_t *pixels = bmalloc((size_t)src_w * 4 * src_h);
	for (size_t i = 0; i < (size_t)src_w * 4 * src_h; i++)
		pixels[i] = (uint8_t)((i * 2654435761u) >> 24);
	const uint8_t *in[1] = {pixels};

	struct frame direct, copied;
	frame_alloc(&direct, dst_w, dst_h, 0);
	frame_alloc(&copied, dst_w, dst_h, 16);

	uint64_t direct_ns = scale_median(scaler, &direct, in, in_linesize);
	uint64_t copied_ns = scale_median(scaler, &copied, in, in_linesize);

	for (int p = 0; p < 2; p++) {
		const uint32_t rows = p == 0 ? dst_h : dst_h / 2;
		for (uint32_t y = 0; y < rows; y++)
			assert_memory_equal(direct.data[p] + (size_t)y * direct.linesize[p],
					    copied.data[p] + (size_t)y * copied.linesize[p], dst_w);
	}

	printf("%s: aligned frame %.3f ms, odd-linesize frame %.3f ms (median of %d)\n", label, (double)direct_ns / 1e6,
	       (double)copied_ns / 1e6, RUNS);

	frame_free(&direct);
	frame_free(&copied);
	bfree(pixels);
	video_scaler_destroy(scaler);
}

static void bgra_to_nv12_1080p(void **state)
{
	(void)state;
	run(1920, 1080, 1920, 1080, "1080p BGRA->NV12");
}

static void bgra_to_nv12_4k(void **state)
{
	(void)state;
	run(3840, 2160, 3840, 2160, "4K BGRA->NV12");
}

static void bgra_4k_to_nv12_1080p(void **state)
{
	(void)state;
	run(3840, 2160, 1920, 1080, "4K->1080p BGRA->NV12");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(bgra_to_nv12_1080p),
		cmocka_unit_test(bgra_to_nv12_4k),
		cmocka_unit_test(bgra_4k_to_nv12_1080p),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
