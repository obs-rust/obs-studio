#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <setjmp.h>
#include <cmocka.h>

#include "media-io/video-scaler-plan.h"

/* Characterization of the swscale scaler's decisions, moved from
 * libobs/media-io/video-scaler-ffmpeg.c to libobs/media-io/video-scaler-plan.c:
 * the scale flags, the coefficient set, the range, the plane heights and the
 * plane copy. No swscale context and no FFmpeg header: the test calls only the
 * moved functions, which are hidden in libobs, so this characterizes that C
 * source. The plane layouts below are the libavutil descriptors' component
 * planes and vertical chroma shifts of the formats the scaler is given. */

/* A height a plane the format does not have keeps. */
#define UNSET (-7)

struct layout {
	const char *name;
	int comp_planes[4];
	uint8_t log2_chroma_h;
};

static const struct layout yuv420p = {"yuv420p", {0, 1, 2, 0}, 1};
static const struct layout yuv422p = {"yuv422p", {0, 1, 2, 0}, 0};
static const struct layout yuv444p = {"yuv444p", {0, 1, 2, 0}, 0};
static const struct layout nv12 = {"nv12", {0, 1, 1, 0}, 1};
static const struct layout p010 = {"p010le", {0, 1, 1, 0}, 1};
static const struct layout yuva420p = {"yuva420p", {0, 1, 2, 3}, 1};
static const struct layout yuva444p = {"yuva444p", {0, 1, 2, 3}, 0};
static const struct layout rgba = {"rgba", {0, 0, 0, 0}, 0};
static const struct layout yuyv422 = {"yuyv422", {0, 0, 0, 0}, 0};

static void check_heights(const struct layout *l, uint32_t height, int h0, int h1, int h2, int h3)
{
	int heights[4] = {UNSET, UNSET, UNSET, UNSET};
	video_scaler_plane_heights(heights, height, l->comp_planes, &l->log2_chroma_h);
	if (heights[0] != h0 || heights[1] != h1 || heights[2] != h2 || heights[3] != h3) {
		fprintf(stderr, "%s at %u: %d %d %d %d, expected %d %d %d %d\n", l->name, height, heights[0],
			heights[1], heights[2], heights[3], h0, h1, h2, h3);
		fail();
	}
}

static void check_copy(size_t scaled_linesize, size_t plane_linesize, size_t height, bool whole, size_t size)
{
	size_t got = (size_t)-1;
	bool got_whole = video_scaler_plane_copy(scaled_linesize, plane_linesize, height, &got);
	if (got_whole != whole || got != size) {
		fprintf(stderr, "copy %zu to %zu, %zu rows: %s of %zu, expected %s of %zu\n", scaled_linesize,
			plane_linesize, height, got_whole ? "whole" : "rows", got, whole ? "whole" : "rows", size);
		fail();
	}
}

static void constants(void **state)
{
	(void)state;
	assert_int_equal(VIDEO_SCALER_SWS_FAST_BILINEAR, 0x01);
	assert_int_equal(VIDEO_SCALER_SWS_BILINEAR, 0x02);
	assert_int_equal(VIDEO_SCALER_SWS_BICUBIC, 0x04);
	assert_int_equal(VIDEO_SCALER_SWS_POINT, 0x10);
	assert_int_equal(VIDEO_SCALER_SWS_AREA, 0x20);
	assert_int_equal(VIDEO_SCALER_SWS_CS_ITU709, 1);
	assert_int_equal(VIDEO_SCALER_SWS_CS_ITU601, 5);
	assert_int_equal(VIDEO_SCALER_SWS_CS_BT2020, 9);
}

static void scale_flags(void **state)
{
	(void)state;
	assert_int_equal(video_scaler_sws_flags(VIDEO_SCALE_DEFAULT), VIDEO_SCALER_SWS_FAST_BILINEAR);
	assert_int_equal(video_scaler_sws_flags(VIDEO_SCALE_POINT), VIDEO_SCALER_SWS_POINT);
	assert_int_equal(video_scaler_sws_flags(VIDEO_SCALE_FAST_BILINEAR), VIDEO_SCALER_SWS_FAST_BILINEAR);
	assert_int_equal(video_scaler_sws_flags(VIDEO_SCALE_BILINEAR),
			 VIDEO_SCALER_SWS_BILINEAR | VIDEO_SCALER_SWS_AREA);
	assert_int_equal(video_scaler_sws_flags(VIDEO_SCALE_BICUBIC), VIDEO_SCALER_SWS_BICUBIC);
	/* characterized, not endorsed: anything outside the enum is point */
	assert_int_equal(video_scaler_sws_flags((enum video_scale_type)(VIDEO_SCALE_BICUBIC + 1)),
			 VIDEO_SCALER_SWS_POINT);
	assert_int_equal(video_scaler_sws_flags((enum video_scale_type) - 1), VIDEO_SCALER_SWS_POINT);
}

static void colorspaces(void **state)
{
	(void)state;
	assert_int_equal(video_scaler_sws_colorspace(VIDEO_CS_DEFAULT), VIDEO_SCALER_SWS_CS_ITU709);
	assert_int_equal(video_scaler_sws_colorspace(VIDEO_CS_601), VIDEO_SCALER_SWS_CS_ITU601);
	assert_int_equal(video_scaler_sws_colorspace(VIDEO_CS_709), VIDEO_SCALER_SWS_CS_ITU709);
	assert_int_equal(video_scaler_sws_colorspace(VIDEO_CS_SRGB), VIDEO_SCALER_SWS_CS_ITU709);
	assert_int_equal(video_scaler_sws_colorspace(VIDEO_CS_2100_PQ), VIDEO_SCALER_SWS_CS_BT2020);
	assert_int_equal(video_scaler_sws_colorspace(VIDEO_CS_2100_HLG), VIDEO_SCALER_SWS_CS_BT2020);
	/* anything outside the enum is 709 */
	assert_int_equal(video_scaler_sws_colorspace((enum video_colorspace)(VIDEO_CS_2100_HLG + 1)),
			 VIDEO_SCALER_SWS_CS_ITU709);
	assert_int_equal(video_scaler_sws_colorspace((enum video_colorspace) - 1), VIDEO_SCALER_SWS_CS_ITU709);
}

static void ranges(void **state)
{
	(void)state;
	assert_int_equal(video_scaler_sws_range(VIDEO_RANGE_DEFAULT), 0);
	assert_int_equal(video_scaler_sws_range(VIDEO_RANGE_PARTIAL), 0);
	assert_int_equal(video_scaler_sws_range(VIDEO_RANGE_FULL), 1);
	/* anything outside the enum is partial */
	assert_int_equal(video_scaler_sws_range((enum video_range_type)(VIDEO_RANGE_FULL + 1)), 0);
	assert_int_equal(video_scaler_sws_range((enum video_range_type) - 1), 0);
}

static void plane_heights_planar(void **state)
{
	(void)state;
	check_heights(&yuv420p, 720, 720, 360, 360, UNSET);
	check_heights(&yuv420p, 1080, 1080, 540, 540, UNSET);
	check_heights(&yuv422p, 720, 720, 720, 720, UNSET);
	check_heights(&yuv422p, 721, 721, 721, 721, UNSET);
	check_heights(&yuv444p, 1080, 1080, 1080, 1080, UNSET);
	check_heights(&yuv444p, 1, 1, 1, 1, UNSET);
}

static void plane_heights_semi_planar(void **state)
{
	(void)state;
	check_heights(&nv12, 720, 720, 360, UNSET, UNSET);
	check_heights(&nv12, 2160, 2160, 1080, UNSET, UNSET);
	check_heights(&p010, 1080, 1080, 540, UNSET, UNSET);
}

static void plane_heights_alpha(void **state)
{
	(void)state;
	/* the alpha plane is never shifted */
	check_heights(&yuva420p, 720, 720, 360, 360, 720);
	check_heights(&yuva444p, 720, 720, 720, 720, 720);
	check_heights(&yuva444p, 719, 719, 719, 719, 719);
}

static void plane_heights_packed(void **state)
{
	(void)state;
	check_heights(&rgba, 720, 720, UNSET, UNSET, UNSET);
	check_heights(&rgba, 721, 721, UNSET, UNSET, UNSET);
	check_heights(&yuyv422, 1080, 1080, UNSET, UNSET, UNSET);
}

static void plane_heights_zero(void **state)
{
	(void)state;
	check_heights(&yuv420p, 0, 0, 0, 0, UNSET);
	check_heights(&yuva420p, 0, 0, 0, 0, 0);
	check_heights(&rgba, 0, 0, UNSET, UNSET, UNSET);
}

static void chroma_shift_read_only_for_chroma(void **state)
{
	(void)state;
	/* With no plane 1 or 2, the shift is never read. */
	int heights[4] = {UNSET, UNSET, UNSET, UNSET};
	const int packed[4] = {0, 0, 0, 0};
	video_scaler_plane_heights(heights, 480, packed, NULL);
	assert_int_equal(heights[0], 480);
	assert_int_equal(heights[1], UNSET);
	assert_int_equal(heights[2], UNSET);
	assert_int_equal(heights[3], UNSET);

	const int alpha_only[4] = {0, 0, 0, 3};
	video_scaler_plane_heights(heights, 480, alpha_only, NULL);
	assert_int_equal(heights[0], 480);
	assert_int_equal(heights[1], UNSET);
	assert_int_equal(heights[2], UNSET);
	assert_int_equal(heights[3], 480);
}

static void chroma_shift_by_two(void **state)
{
	(void)state;
	/* yuv410p: a quarter of the height */
	const struct layout yuv410p = {"yuv410p", {0, 1, 2, 0}, 2};
	check_heights(&yuv410p, 720, 720, 180, 180, UNSET);
}

static void copy_equal_linesizes(void **state)
{
	(void)state;
	check_copy(1920, 1920, 1080, true, 1920 * 1080);
	check_copy(960, 960, 540, true, 960 * 540);
	check_copy(1, 1, 1, true, 1);
}

static void copy_wider_scaled(void **state)
{
	(void)state;
	/* the scaled plane is padded to 32 bytes: one row of the output's width */
	check_copy(1952, 1940, 1080, false, 1940);
	check_copy(992, 970, 540, false, 970);
}

static void copy_wider_output(void **state)
{
	(void)state;
	/* the output plane is wider: one row of the scaled width */
	check_copy(1920, 2048, 1080, false, 1920);
	check_copy(32, 4096, 1, false, 32);
}

static void copy_zero_height(void **state)
{
	(void)state;
	check_copy(1920, 1920, 0, true, 0);
	check_copy(1952, 1920, 0, false, 1920);
	check_copy(0, 0, 720, true, 0);
	check_copy(0, 64, 720, false, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(scale_flags),
		cmocka_unit_test(colorspaces),
		cmocka_unit_test(ranges),
		cmocka_unit_test(plane_heights_planar),
		cmocka_unit_test(plane_heights_semi_planar),
		cmocka_unit_test(plane_heights_alpha),
		cmocka_unit_test(plane_heights_packed),
		cmocka_unit_test(plane_heights_zero),
		cmocka_unit_test(chroma_shift_read_only_for_chroma),
		cmocka_unit_test(chroma_shift_by_two),
		cmocka_unit_test(copy_equal_linesizes),
		cmocka_unit_test(copy_wider_scaled),
		cmocka_unit_test(copy_wider_output),
		cmocka_unit_test(copy_zero_height),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
