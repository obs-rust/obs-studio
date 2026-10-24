#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-video-copy-plan.h"

/*
 * Characterization of the output frame copy plan and the scale effect choice
 * of libobs/obs-video.c, moved to libobs/obs-video-copy-plan.c before their
 * Rust port:
 * - obs_video_gpu_copy_plan(): the copies set_gpu_converted_data() makes
 *   from a GPU-converted frame into the output frame, per format: each
 *   plane's row size, rows and strides, and whether it is one copy;
 * - obs_video_rgbx_copy_plan(): the copy copy_rgbx_frame() makes;
 * - obs_video_scale_resolution_close(): resolution_close();
 * - obs_video_scale_effect_choice(): get_scale_effect(), with
 *   get_scale_effect_internal() and the fallback when an effect did not
 *   load;
 * - obs_video_mix_reuse_match(): one comparison of can_reuse_mix_texture().
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header. obs-video.c runs
 * the copies (every memcpy() stays there); the real-libobs tests
 * (test_video_output_formats, every GPU-converted format read back plane by
 * plane; test_scaled_mix_alpha, a reused mix texture; test_encoding_lag_rescale,
 * a rescaled encoder's own mix; test_video_output_plan) cover the callers.
 *
 * Nothing here comes from outside OBS: the format and sizes come from
 * obs_reset_video() (the frontend, plugins and scripts), the line sizes from
 * the graphics driver's mapped surfaces and video-io's frames.
 */

#define SCALE_DEFAULT OBS_VIDEO_SCALE_EFFECT_DEFAULT
#define SCALE_LOWRES OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES
#define SCALE_BICUBIC OBS_VIDEO_SCALE_EFFECT_BICUBIC
#define SCALE_LANCZOS OBS_VIDEO_SCALE_EFFECT_LANCZOS
#define SCALE_AREA OBS_VIDEO_SCALE_EFFECT_AREA
#define LOADED_ALL 0x1fu

/* ------------------------------------------------------------------------ */
/* helpers */

static void check_copy(const char *what, size_t index, const struct obs_video_copy *got, bool after, bool whole,
		       size_t bytes, uint32_t rows, uint32_t src_stride, uint32_t dst_stride)
{
	if (got->src_after_plane0 != after || got->whole != whole || got->bytes != bytes || got->rows != rows ||
	    got->src_stride != src_stride || got->dst_stride != dst_stride) {
		fprintf(stderr,
			"%s copy %zu: after %d whole %d bytes %zu rows %u strides %u/%u, "
			"expected after %d whole %d bytes %zu rows %u strides %u/%u\n",
			what, index, got->src_after_plane0, got->whole, got->bytes, got->rows, got->src_stride,
			got->dst_stride, after, whole, bytes, rows, src_stride, dst_stride);
		fail();
	}
}

static void lines(uint32_t out[MAX_AV_PLANES], uint32_t l0, uint32_t l1, uint32_t l2)
{
	memset(out, 0, sizeof(uint32_t) * MAX_AV_PLANES);
	out[0] = l0;
	out[1] = l1;
	out[2] = l2;
}

static struct obs_video_copy_plan plan_of(enum video_format format, uint32_t width, uint32_t height, const uint32_t *in,
					  const uint32_t *out)
{
	struct obs_video_copy_plan plan = obs_video_gpu_copy_plan(format, width, height, in, out);
	if (plan.count > OBS_VIDEO_COPY_MAX) {
		fprintf(stderr, "format %d: %zu copies\n", (int)format, plan.count);
		fail();
	}
	return plan;
}

/* The shape of one copy of a format: its row size and rows from the frame's
 * width and height (as the C computes them, in uint32_t), and which line
 * sizes it reads. */
enum row_size { ROW_W, ROW_W_D2, ROW_W_X2, ROW_W_X4 };
enum row_count { ROWS_H, ROWS_H_D2 };

struct shape {
	enum row_size row;
	enum row_count rows;
	size_t in_line; /* input linesize[] index for the source stride */
	size_t out_line;
	bool after;
};

struct format_shape {
	enum video_format format;
	size_t count;
	struct shape copies[OBS_VIDEO_COPY_MAX];
};

/* Every format with a GPU conversion, with input linesize[1] set, then the
 * packed NV12 and P010 (input linesize[1] of 0). */
static const struct format_shape shapes[] = {
	{VIDEO_FORMAT_I420,
	 3,
	 {{ROW_W, ROWS_H, 0, 0, false}, {ROW_W_D2, ROWS_H_D2, 1, 1, false}, {ROW_W_D2, ROWS_H_D2, 2, 2, false}}},
	{VIDEO_FORMAT_NV12, 2, {{ROW_W, ROWS_H, 0, 0, false}, {ROW_W, ROWS_H_D2, 1, 1, false}}},
	{VIDEO_FORMAT_I444,
	 3,
	 {{ROW_W, ROWS_H, 0, 0, false}, {ROW_W, ROWS_H, 1, 1, false}, {ROW_W, ROWS_H, 2, 2, false}}},
	{VIDEO_FORMAT_I010,
	 3,
	 {{ROW_W_X2, ROWS_H, 0, 0, false}, {ROW_W, ROWS_H_D2, 1, 1, false}, {ROW_W, ROWS_H_D2, 2, 2, false}}},
	{VIDEO_FORMAT_P010, 2, {{ROW_W_X2, ROWS_H, 0, 0, false}, {ROW_W_X2, ROWS_H_D2, 1, 1, false}}},
	{VIDEO_FORMAT_P216, 2, {{ROW_W_X2, ROWS_H, 0, 0, false}, {ROW_W_X2, ROWS_H, 1, 1, false}}},
	{VIDEO_FORMAT_P416, 2, {{ROW_W_X2, ROWS_H, 0, 0, false}, {ROW_W_X4, ROWS_H, 1, 1, false}}},
};

static const struct format_shape packed_shapes[] = {
	{VIDEO_FORMAT_NV12, 2, {{ROW_W, ROWS_H, 0, 0, false}, {ROW_W, ROWS_H_D2, 0, 1, true}}},
	{VIDEO_FORMAT_P010, 2, {{ROW_W_X2, ROWS_H, 0, 0, false}, {ROW_W_X2, ROWS_H_D2, 0, 1, true}}},
};

#define COUNT(a) (sizeof(a) / sizeof((a)[0]))

static uint32_t row_bytes(enum row_size row, uint32_t width)
{
	switch (row) {
	case ROW_W_D2:
		return width / 2;
	case ROW_W_X2:
		return width * 2;
	case ROW_W_X4:
		return width * 4;
	case ROW_W:
	default:
		return width;
	}
}

static uint32_t row_count(enum row_count rows, uint32_t height)
{
	return rows == ROWS_H_D2 ? height / 2 : height;
}

/* The plan of fs at width x height with the line sizes in and out matches
 * its shape: one copy of row * rows when the row size equals both line
 * sizes, else rows of the row size. */
static void check_shape(const struct format_shape *fs, uint32_t width, uint32_t height, const uint32_t *in,
			const uint32_t *out)
{
	char what[96];
	snprintf(what, sizeof(what), "format %d %ux%u in %u,%u,%u out %u,%u,%u", (int)fs->format, width, height, in[0],
		 in[1], in[2], out[0], out[1], out[2]);

	struct obs_video_copy_plan plan = plan_of(fs->format, width, height, in, out);
	if (plan.count != fs->count) {
		fprintf(stderr, "%s: %zu copies, expected %zu\n", what, plan.count, fs->count);
		fail();
	}
	for (size_t i = 0; i < fs->count; i++) {
		const struct shape *s = &fs->copies[i];
		uint32_t row = row_bytes(s->row, width);
		uint32_t rows = row_count(s->rows, height);
		uint32_t src = in[s->in_line];
		uint32_t dst = out[s->out_line];
		bool whole = row == src && row == dst;
		size_t bytes = whole ? (size_t)row * (size_t)rows : (size_t)row;
		check_copy(what, i, &plan.copies[i], s->after, whole, bytes, rows, src, dst);
	}
}

static bool has_gpu_copy(enum video_format format)
{
	for (size_t i = 0; i < COUNT(shapes); i++)
		if (shapes[i].format == format)
			return true;
	return false;
}

/* ------------------------------------------------------------------------ */
/* constants */

static void constants(void **state)
{
	(void)state;
	assert_int_equal(OBS_VIDEO_COPY_MAX, 3);
	assert_int_equal(OBS_VIDEO_COPY_SCALE_DISABLE, 0);
	assert_int_equal(OBS_VIDEO_COPY_SCALE_POINT, 1);
	assert_int_equal(OBS_VIDEO_COPY_SCALE_BICUBIC, 2);
	assert_int_equal(OBS_VIDEO_COPY_SCALE_BILINEAR, 3);
	assert_int_equal(OBS_VIDEO_COPY_SCALE_LANCZOS, 4);
	assert_int_equal(OBS_VIDEO_COPY_SCALE_AREA, 5);
	assert_int_equal(SCALE_DEFAULT, 0);
	assert_int_equal(SCALE_LOWRES, 1);
	assert_int_equal(SCALE_BICUBIC, 2);
	assert_int_equal(SCALE_LANCZOS, 3);
	assert_int_equal(SCALE_AREA, 4);
	assert_int_equal(OBS_VIDEO_SCALE_LOADED(SCALE_DEFAULT), 0x1);
	assert_int_equal(OBS_VIDEO_SCALE_LOADED(SCALE_AREA), 0x10);
}

/* ------------------------------------------------------------------------ */
/* obs_video_gpu_copy_plan */

static void gpu_formats_without_copy(void **state)
{
	(void)state;
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	/* every format without a GPU conversion: no copy, whatever the sizes */
	for (int f = VIDEO_FORMAT_NONE; f <= VIDEO_FORMAT_R10L; f++) {
		if (has_gpu_copy((enum video_format)f))
			continue;
		lines(in, 1920, 1920, 1920);
		lines(out, 1920, 1920, 1920);
		assert_int_equal(plan_of((enum video_format)f, 1920, 1080, in, out).count, 0);
		lines(in, 0, 0, 0);
		lines(out, 7680, 3840, 3840);
		assert_int_equal(plan_of((enum video_format)f, 1920, 1080, in, out).count, 0);
	}
	/* and two values outside the enum */
	lines(in, 1920, 1920, 1920);
	lines(out, 1920, 1920, 1920);
	assert_int_equal(plan_of((enum video_format)(VIDEO_FORMAT_R10L + 1), 1920, 1080, in, out).count, 0);
	assert_int_equal(plan_of((enum video_format)0x7fffffff, 1920, 1080, in, out).count, 0);
}

static void gpu_every_format_has_its_copies(void **state)
{
	(void)state;
	/* the count of each format in the enum */
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	lines(in, 4096, 4096, 4096);
	lines(out, 1920, 1920, 1920);
	for (int f = VIDEO_FORMAT_NONE; f <= VIDEO_FORMAT_R10L; f++) {
		size_t want = 0;
		for (size_t i = 0; i < COUNT(shapes); i++)
			if (shapes[i].format == (enum video_format)f)
				want = shapes[i].count;
		struct obs_video_copy_plan plan = plan_of((enum video_format)f, 1920, 1080, in, out);
		if (plan.count != want) {
			fprintf(stderr, "format %d: %zu copies, expected %zu\n", f, plan.count, want);
			fail();
		}
	}
}

static void gpu_i420(void **state)
{
	(void)state;
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	/* tight planes: three single copies */
	lines(in, 1920, 960, 960);
	lines(out, 1920, 960, 960);
	struct obs_video_copy_plan p = plan_of(VIDEO_FORMAT_I420, 1920, 1080, in, out);
	assert_int_equal(p.count, 3);
	check_copy("i420", 0, &p.copies[0], false, true, (size_t)1920 * 1080, 1080, 1920, 1920);
	check_copy("i420", 1, &p.copies[1], false, true, (size_t)960 * 540, 540, 960, 960);
	check_copy("i420", 2, &p.copies[2], false, true, (size_t)960 * 540, 540, 960, 960);

	/* padded input rows (a mapped surface's pitch): rows */
	lines(in, 2048, 1024, 1024);
	p = plan_of(VIDEO_FORMAT_I420, 1920, 1080, in, out);
	check_copy("i420 padded", 0, &p.copies[0], false, false, 1920, 1080, 2048, 1920);
	check_copy("i420 padded", 1, &p.copies[1], false, false, 960, 540, 1024, 960);
	check_copy("i420 padded", 2, &p.copies[2], false, false, 960, 540, 1024, 960);
}

static void gpu_nv12(void **state)
{
	(void)state;
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	/* two textures: the UV plane from input plane 1 */
	lines(in, 1920, 1920, 0);
	lines(out, 1920, 1920, 0);
	struct obs_video_copy_plan p = plan_of(VIDEO_FORMAT_NV12, 1920, 1080, in, out);
	assert_int_equal(p.count, 2);
	check_copy("nv12", 0, &p.copies[0], false, true, (size_t)1920 * 1080, 1080, 1920, 1920);
	check_copy("nv12", 1, &p.copies[1], false, true, (size_t)1920 * 540, 540, 1920, 1920);

	lines(in, 2048, 2560, 0);
	p = plan_of(VIDEO_FORMAT_NV12, 1920, 1080, in, out);
	check_copy("nv12 padded", 0, &p.copies[0], false, false, 1920, 1080, 2048, 1920);
	check_copy("nv12 padded", 1, &p.copies[1], false, false, 1920, 540, 2560, 1920);
}

static void gpu_nv12_packed(void **state)
{
	(void)state;
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	/* one NV12 texture (input linesize[1] of 0): the UV plane follows
	 * plane 0 in input plane 0, at its line size */
	lines(in, 1920, 0, 0);
	lines(out, 1920, 1920, 0);
	struct obs_video_copy_plan p = plan_of(VIDEO_FORMAT_NV12, 1920, 1080, in, out);
	assert_int_equal(p.count, 2);
	check_copy("nv12 packed", 0, &p.copies[0], false, true, (size_t)1920 * 1080, 1080, 1920, 1920);
	check_copy("nv12 packed", 1, &p.copies[1], true, true, (size_t)1920 * 540, 540, 1920, 1920);

	lines(in, 2048, 0, 0);
	p = plan_of(VIDEO_FORMAT_NV12, 1920, 1080, in, out);
	check_copy("nv12 packed padded", 0, &p.copies[0], false, false, 1920, 1080, 2048, 1920);
	check_copy("nv12 packed padded", 1, &p.copies[1], true, false, 1920, 540, 2048, 1920);

	/* input linesize[2] does not matter, only [1] */
	lines(in, 1920, 0, 1920);
	p = plan_of(VIDEO_FORMAT_NV12, 1920, 1080, in, out);
	assert_true(p.copies[1].src_after_plane0);
}

static void gpu_i444(void **state)
{
	(void)state;
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	lines(in, 1280, 1280, 1280);
	lines(out, 1280, 1280, 1280);
	struct obs_video_copy_plan p = plan_of(VIDEO_FORMAT_I444, 1280, 720, in, out);
	assert_int_equal(p.count, 3);
	for (size_t i = 0; i < 3; i++)
		check_copy("i444", i, &p.copies[i], false, true, (size_t)1280 * 720, 720, 1280, 1280);
	lines(out, 1280, 1344, 1280);
	p = plan_of(VIDEO_FORMAT_I444, 1280, 720, in, out);
	check_copy("i444", 0, &p.copies[0], false, true, (size_t)1280 * 720, 720, 1280, 1280);
	check_copy("i444", 1, &p.copies[1], false, false, 1280, 720, 1280, 1344);
	check_copy("i444", 2, &p.copies[2], false, true, (size_t)1280 * 720, 720, 1280, 1280);
}

static void gpu_i010(void **state)
{
	(void)state;
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	/* 16-bit samples: plane 0's rows are width * 2 bytes, the chroma
	 * planes' width bytes (half the samples, two bytes each) */
	lines(in, 3840, 1920, 1920);
	lines(out, 3840, 1920, 1920);
	struct obs_video_copy_plan p = plan_of(VIDEO_FORMAT_I010, 1920, 1080, in, out);
	assert_int_equal(p.count, 3);
	check_copy("i010", 0, &p.copies[0], false, true, (size_t)3840 * 1080, 1080, 3840, 3840);
	check_copy("i010", 1, &p.copies[1], false, true, (size_t)1920 * 540, 540, 1920, 1920);
	check_copy("i010", 2, &p.copies[2], false, true, (size_t)1920 * 540, 540, 1920, 1920);
}

static void gpu_p010(void **state)
{
	(void)state;
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	lines(in, 3840, 3840, 0);
	lines(out, 3840, 3840, 0);
	struct obs_video_copy_plan p = plan_of(VIDEO_FORMAT_P010, 1920, 1080, in, out);
	assert_int_equal(p.count, 2);
	check_copy("p010", 0, &p.copies[0], false, true, (size_t)3840 * 1080, 1080, 3840, 3840);
	check_copy("p010", 1, &p.copies[1], false, true, (size_t)3840 * 540, 540, 3840, 3840);

	/* one P010 texture: as packed NV12 */
	lines(in, 4096, 0, 0);
	p = plan_of(VIDEO_FORMAT_P010, 1920, 1080, in, out);
	check_copy("p010 packed", 0, &p.copies[0], false, false, 3840, 1080, 4096, 3840);
	check_copy("p010 packed", 1, &p.copies[1], true, false, 3840, 540, 4096, 3840);
	lines(in, 3840, 0, 0);
	p = plan_of(VIDEO_FORMAT_P010, 1920, 1080, in, out);
	check_copy("p010 packed tight", 1, &p.copies[1], true, true, (size_t)3840 * 540, 540, 3840, 3840);
}

static void gpu_p216_p416(void **state)
{
	(void)state;
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	/* P216: both planes width * 2 bytes and full height */
	lines(in, 3840, 3840, 0);
	lines(out, 3840, 3840, 0);
	struct obs_video_copy_plan p = plan_of(VIDEO_FORMAT_P216, 1920, 1080, in, out);
	assert_int_equal(p.count, 2);
	check_copy("p216", 0, &p.copies[0], false, true, (size_t)3840 * 1080, 1080, 3840, 3840);
	check_copy("p216", 1, &p.copies[1], false, true, (size_t)3840 * 1080, 1080, 3840, 3840);
	/* input linesize[1] of 0 is not a packed frame here */
	lines(in, 3840, 0, 0);
	p = plan_of(VIDEO_FORMAT_P216, 1920, 1080, in, out);
	check_copy("p216 zero", 1, &p.copies[1], false, false, 3840, 1080, 0, 3840);

	/* P416: plane 1 is width * 4 bytes */
	lines(in, 3840, 7680, 0);
	lines(out, 3840, 7680, 0);
	p = plan_of(VIDEO_FORMAT_P416, 1920, 1080, in, out);
	assert_int_equal(p.count, 2);
	check_copy("p416", 0, &p.copies[0], false, true, (size_t)3840 * 1080, 1080, 3840, 3840);
	check_copy("p416", 1, &p.copies[1], false, true, (size_t)7680 * 1080, 1080, 7680, 7680);
}

/* Each line size, in or out, decides only the copies that read it. */
static void gpu_each_line_size(void **state)
{
	(void)state;
	const uint32_t w = 640, h = 360;
	for (int packed = 0; packed < 2; packed++) {
		const struct format_shape *list = packed ? packed_shapes : shapes;
		size_t n = packed ? COUNT(packed_shapes) : COUNT(shapes);
		for (size_t f = 0; f < n; f++) {
			const struct format_shape *fs = &list[f];
			uint32_t tight_in[MAX_AV_PLANES] = {0}, tight_out[MAX_AV_PLANES] = {0};
			for (size_t i = 0; i < fs->count; i++) {
				tight_in[fs->copies[i].in_line] = row_bytes(fs->copies[i].row, w);
				tight_out[fs->copies[i].out_line] = row_bytes(fs->copies[i].row, w);
			}
			if (!packed && (fs->format == VIDEO_FORMAT_NV12 || fs->format == VIDEO_FORMAT_P010) &&
			    tight_in[1] == 0)
				tight_in[1] = 1;
			check_shape(fs, w, h, tight_in, tight_out);
			for (size_t line = 0; line < 3; line++) {
				for (int side = 0; side < 2; side++) {
					uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
					memcpy(in, tight_in, sizeof(in));
					memcpy(out, tight_out, sizeof(out));
					uint32_t *l = side ? out : in;
					if (packed && !side && line == 1)
						continue; /* stays 0: packed */
					l[line] += 64;
					check_shape(fs, w, h, in, out);
				}
			}
		}
	}
}

static void gpu_widths_and_heights(void **state)
{
	(void)state;
	/* 0, 1, odd and even sizes: the halves round down */
	const uint32_t widths[] = {0, 1, 2, 3, 639, 640, 1919, 1921};
	const uint32_t heights[] = {0, 1, 2, 3, 359, 1081};
	for (size_t wi = 0; wi < COUNT(widths); wi++) {
		for (size_t hi = 0; hi < COUNT(heights); hi++) {
			uint32_t w = widths[wi], h = heights[hi];
			for (int packed = 0; packed < 2; packed++) {
				const struct format_shape *list = packed ? packed_shapes : shapes;
				size_t n = packed ? COUNT(packed_shapes) : COUNT(shapes);
				for (size_t f = 0; f < n; f++) {
					const struct format_shape *fs = &list[f];
					uint32_t in[MAX_AV_PLANES] = {0}, out[MAX_AV_PLANES] = {0};
					/* tight */
					for (size_t i = 0; i < fs->count; i++) {
						in[fs->copies[i].in_line] = row_bytes(fs->copies[i].row, w);
						out[fs->copies[i].out_line] = row_bytes(fs->copies[i].row, w);
					}
					if (!packed && in[1] == 0)
						in[1] = 1;
					check_shape(fs, w, h, in, out);
					/* padded to 64 bytes */
					for (size_t l = 0; l < 3; l++) {
						in[l] = (in[l] + 64) & ~63u;
						out[l] = (out[l] + 32) & ~31u;
					}
					if (packed)
						in[1] = 0;
					check_shape(fs, w, h, in, out);
				}
			}
		}
	}

	/* a width of 0 with line sizes of 0 is one copy of 0 bytes per plane */
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	lines(in, 0, 1, 0);
	lines(out, 0, 0, 0);
	struct obs_video_copy_plan p = plan_of(VIDEO_FORMAT_NV12, 0, 1080, in, out);
	check_copy("nv12 0 wide", 0, &p.copies[0], false, true, 0, 1080, 0, 0);
	check_copy("nv12 0 wide", 1, &p.copies[1], false, false, 0, 540, 1, 0);
}

/* Characterized, not endorsed: the row sizes are uint32_t, so width * 2 and
 * width * 4 wrap; the single copy's size is a size_t product. No real frame
 * is this wide (obs_reset_video() allows 32768). */
static void gpu_width_wrap(void **state)
{
	(void)state;
	const uint32_t widths[] = {0x40000000u, 0x7fffffffu, 0x80000000u, 0x80000001u, 0xffffffffu};
	const uint32_t heights[] = {0, 2, 0xfffffffeu, 0xffffffffu};
	for (size_t wi = 0; wi < COUNT(widths); wi++) {
		for (size_t hi = 0; hi < COUNT(heights); hi++) {
			uint32_t w = widths[wi], h = heights[hi];
			for (size_t f = 0; f < COUNT(shapes); f++) {
				const struct format_shape *fs = &shapes[f];
				uint32_t in[MAX_AV_PLANES] = {0}, out[MAX_AV_PLANES] = {0};
				for (size_t i = 0; i < fs->count; i++) {
					in[fs->copies[i].in_line] = row_bytes(fs->copies[i].row, w);
					out[fs->copies[i].out_line] = row_bytes(fs->copies[i].row, w);
				}
				if (in[1] == 0)
					in[1] = 1;
				check_shape(fs, w, h, in, out);
				lines(in, 1, 1, 1);
				lines(out, 2, 2, 2);
				check_shape(fs, w, h, in, out);
			}
		}
	}

	/* the wraps themselves */
	uint32_t in[MAX_AV_PLANES], out[MAX_AV_PLANES];
	lines(in, 1, 1, 1);
	lines(out, 1, 1, 1);
	struct obs_video_copy_plan p = plan_of(VIDEO_FORMAT_P416, 0x40000000u, 4, in, out);
	assert_int_equal(p.copies[0].bytes, 0x80000000u);
	assert_int_equal(p.copies[1].bytes, 0); /* 0x40000000 * 4 */
	p = plan_of(VIDEO_FORMAT_I010, 0x80000000u, 4, in, out);
	assert_int_equal(p.copies[0].bytes, 0); /* 0x80000000 * 2 */
	assert_int_equal(p.copies[1].bytes, 0x80000000u);
	p = plan_of(VIDEO_FORMAT_P010, 0x80000001u, 4, in, out);
	assert_int_equal(p.copies[0].bytes, 2);
	p = plan_of(VIDEO_FORMAT_P216, 0xffffffffu, 4, in, out);
	assert_int_equal(p.copies[1].bytes, 0xfffffffeu);
	p = plan_of(VIDEO_FORMAT_I420, 0xffffffffu, 4, in, out);
	assert_int_equal(p.copies[1].bytes, 0x7fffffffu);

	/* a wrapped row size of 0 matches line sizes of 0: one copy of 0 */
	lines(in, 0, 0, 0);
	lines(out, 0, 0, 0);
	p = plan_of(VIDEO_FORMAT_P416, 0x40000000u, 1080, in, out);
	check_copy("p416 wrap", 1, &p.copies[1], false, true, 0, 1080, 0, 0);

	/* the single copy's size does not wrap in uint32_t */
	lines(in, 0xffffffffu, 0, 0);
	lines(out, 0xffffffffu, 0, 0);
	p = plan_of(VIDEO_FORMAT_I444, 0xffffffffu, 0xffffffffu, in, out);
	check_copy("i444 max", 0, &p.copies[0], false, true, (size_t)0xffffffffu * (size_t)0xffffffffu, 0xffffffffu,
		   0xffffffffu, 0xffffffffu);
}

/* ------------------------------------------------------------------------ */
/* obs_video_rgbx_copy_plan */

static void rgbx_equal_line_sizes(void **state)
{
	(void)state;
	struct obs_video_copy c = obs_video_rgbx_copy_plan(1920, 1080, 7680, 7680);
	check_copy("rgbx", 0, &c, false, true, (size_t)7680 * 1080, 1080, 7680, 7680);
	/* padded rows on both sides: still one copy, of the line size, not the
	 * width */
	c = obs_video_rgbx_copy_plan(1920, 1080, 8192, 8192);
	check_copy("rgbx padded", 0, &c, false, true, (size_t)8192 * 1080, 1080, 8192, 8192);
	/* characterized, not endorsed: line sizes under width * 4 that match
	 * are one copy of line size * height */
	c = obs_video_rgbx_copy_plan(1920, 1080, 1920, 1920);
	check_copy("rgbx short", 0, &c, false, true, (size_t)1920 * 1080, 1080, 1920, 1920);
	c = obs_video_rgbx_copy_plan(1920, 1080, 0, 0);
	check_copy("rgbx zero", 0, &c, false, true, 0, 1080, 0, 0);
	c = obs_video_rgbx_copy_plan(0xffffffffu, 0xffffffffu, 0xffffffffu, 0xffffffffu);
	check_copy("rgbx max", 0, &c, false, true, (size_t)0xffffffffu * (size_t)0xffffffffu, 0xffffffffu, 0xffffffffu,
		   0xffffffffu);
}

static void rgbx_rows(void **state)
{
	(void)state;
	struct obs_video_copy c = obs_video_rgbx_copy_plan(1920, 1080, 8192, 7680);
	check_copy("rgbx rows", 0, &c, false, false, (size_t)1920 * 4, 1080, 8192, 7680);
	c = obs_video_rgbx_copy_plan(0, 0, 1, 2);
	check_copy("rgbx 0", 0, &c, false, false, 0, 0, 1, 2);
	c = obs_video_rgbx_copy_plan(1, 3, 4, 8);
	check_copy("rgbx 1", 0, &c, false, false, 4, 3, 4, 8);
	c = obs_video_rgbx_copy_plan(641, 361, 2564, 2576);
	check_copy("rgbx odd", 0, &c, false, false, 2564, 361, 2564, 2576);
	/* width * 4 is a size_t product: it does not wrap at 2^32 */
	c = obs_video_rgbx_copy_plan(0x40000000u, 2, 0, 1);
	check_copy("rgbx wide", 0, &c, false, false, (size_t)0x40000000u * 4, 2, 0, 1);
	c = obs_video_rgbx_copy_plan(0xffffffffu, 2, 0, 1);
	check_copy("rgbx max", 0, &c, false, false, (size_t)0xffffffffu * 4, 2, 0, 1);
}

/* ------------------------------------------------------------------------ */
/* obs_video_scale_resolution_close */

static void resolution_close_edges(void **state)
{
	(void)state;
	/* within 16 on both axes, either side */
	const int deltas[] = {-17, -16, -15, 0, 15, 16, 17};
	for (size_t i = 0; i < COUNT(deltas); i++) {
		int d = deltas[i];
		bool want = d >= -16 && d <= 16;
		/* the width alone, the height alone, both */
		assert_int_equal(obs_video_scale_resolution_close(1920, 1080, (uint32_t)(1920 + d), 1080), want);
		assert_int_equal(obs_video_scale_resolution_close(1920, 1080, 1920, (uint32_t)(1080 + d)), want);
		assert_int_equal(
			obs_video_scale_resolution_close(1920, 1080, (uint32_t)(1920 + d), (uint32_t)(1080 + d)), want);
		/* one axis close is not enough */
		assert_false(obs_video_scale_resolution_close(1920, 1080, (uint32_t)(1920 + d), 720));
		assert_false(obs_video_scale_resolution_close(1920, 1080, 1280, (uint32_t)(1080 + d)));
	}
	/* the sizes obs_reset_video() allows: 2 to 32768 */
	assert_true(obs_video_scale_resolution_close(2, 2, 18, 18));
	assert_false(obs_video_scale_resolution_close(2, 2, 19, 2));
	assert_true(obs_video_scale_resolution_close(32768, 32768, 32752, 32752));
	assert_false(obs_video_scale_resolution_close(32768, 32768, 32751, 32768));
	assert_false(obs_video_scale_resolution_close(32768, 2, 2, 2));
	assert_true(obs_video_scale_resolution_close(0, 0, 16, 16));
	assert_false(obs_video_scale_resolution_close(0, 0, 17, 0));
}

/* ------------------------------------------------------------------------ */
/* obs_video_scale_effect_choice */

static enum obs_video_scale_effect choose(uint32_t base_w, uint32_t base_h, uint32_t target_w, uint32_t target_h,
					  uint32_t out_w, uint32_t out_h, int type, uint32_t loaded)
{
	struct obs_video_scale_input in = {
		.base_width = base_w,
		.base_height = base_h,
		.target_width = target_w,
		.target_height = target_h,
		.output_width = out_w,
		.output_height = out_h,
		.scale_type = type,
		.loaded = loaded,
	};
	return obs_video_scale_effect_choice(&in);
}

static const int every_type[] = {OBS_VIDEO_COPY_SCALE_DISABLE,
				 OBS_VIDEO_COPY_SCALE_POINT,
				 OBS_VIDEO_COPY_SCALE_BICUBIC,
				 OBS_VIDEO_COPY_SCALE_BILINEAR,
				 OBS_VIDEO_COPY_SCALE_LANCZOS,
				 OBS_VIDEO_COPY_SCALE_AREA,
				 -1,
				 6,
				 0x7fffffff};

/* The effect a scale type picks when everything loaded and the output is
 * not low resolution. */
static enum obs_video_scale_effect type_effect(int type)
{
	switch (type) {
	case OBS_VIDEO_COPY_SCALE_BILINEAR:
		return SCALE_DEFAULT;
	case OBS_VIDEO_COPY_SCALE_LANCZOS:
		return SCALE_LANCZOS;
	case OBS_VIDEO_COPY_SCALE_AREA:
		return SCALE_AREA;
	default:
		/* bicubic, and disable, point and values outside the enum */
		return SCALE_BICUBIC;
	}
}

static void scale_close_is_default(void **state)
{
	(void)state;
	/* close to the base size: the default effect, whatever the type, the
	 * output's size and what loaded (even when the default did not) */
	for (size_t t = 0; t < COUNT(every_type); t++)
		for (uint32_t loaded = 0; loaded <= LOADED_ALL; loaded++) {
			assert_int_equal(choose(1920, 1080, 1904, 1096, 1920, 1080, every_type[t], loaded),
					 SCALE_DEFAULT);
			assert_int_equal(choose(1920, 1080, 1920, 1080, 100, 100, every_type[t], loaded),
					 SCALE_DEFAULT);
			assert_int_equal(choose(1920, 1080, 1936, 1064, 0, 0, every_type[t], loaded), SCALE_DEFAULT);
		}
}

static void scale_type_table(void **state)
{
	(void)state;
	for (size_t t = 0; t < COUNT(every_type); t++) {
		/* 1920x1080 to 1280x720: not low resolution */
		assert_int_equal(choose(1920, 1080, 1280, 720, 1280, 720, every_type[t], LOADED_ALL),
				 type_effect(every_type[t]));
		/* to 640x360: under half on both axes, bilinear low resolution
		 * whatever the type */
		assert_int_equal(choose(1920, 1080, 640, 360, 640, 360, every_type[t], LOADED_ALL), SCALE_LOWRES);
		/* upscaling */
		assert_int_equal(choose(1280, 720, 1920, 1080, 1920, 1080, every_type[t], LOADED_ALL),
				 type_effect(every_type[t]));
	}
}

static void scale_lowres_rule(void **state)
{
	(void)state;
	const int type = OBS_VIDEO_COPY_SCALE_LANCZOS;
	/* strictly under half, on both axes, of the output's size (not the
	 * target's) */
	assert_int_equal(choose(1920, 1080, 959, 539, 959, 539, type, LOADED_ALL), SCALE_LOWRES);
	assert_int_equal(choose(1920, 1080, 960, 539, 960, 539, type, LOADED_ALL), SCALE_LANCZOS);
	assert_int_equal(choose(1920, 1080, 959, 540, 959, 540, type, LOADED_ALL), SCALE_LANCZOS);
	assert_int_equal(choose(1920, 1080, 959, 1080, 959, 1080, type, LOADED_ALL), SCALE_LANCZOS);
	/* the halves round down: 1921 / 2 is 960 */
	assert_int_equal(choose(1921, 1081, 959, 539, 959, 539, type, LOADED_ALL), SCALE_LOWRES);
	assert_int_equal(choose(1921, 1081, 960, 539, 960, 539, type, LOADED_ALL), SCALE_LANCZOS);
	/* the output's size decides, not the target's */
	assert_int_equal(choose(1920, 1080, 1280, 720, 640, 360, type, LOADED_ALL), SCALE_LOWRES);
	assert_int_equal(choose(1920, 1080, 640, 360, 1280, 720, type, LOADED_ALL), SCALE_LANCZOS);
	/* a 0 output is under half of any base of 2 or more */
	assert_int_equal(choose(1920, 1080, 640, 360, 0, 0, type, LOADED_ALL), SCALE_LOWRES);
	/* a base of 0 or 1 is never low resolution (its half is 0) */
	assert_int_equal(choose(1, 1, 100, 100, 0, 0, type, LOADED_ALL), SCALE_LANCZOS);
	assert_int_equal(choose(0, 1080, 100, 100, 0, 0, type, LOADED_ALL), SCALE_LANCZOS);
}

/* The fallback when the picked effect did not load: bicubic if it loaded,
 * else the default (which may not have loaded either). */
static enum obs_video_scale_effect expected(int type, bool lowres, uint32_t loaded)
{
	enum obs_video_scale_effect pick = lowres ? SCALE_LOWRES : type_effect(type);
	if (loaded & OBS_VIDEO_SCALE_LOADED(pick))
		return pick;
	return (loaded & OBS_VIDEO_SCALE_LOADED(SCALE_BICUBIC)) ? SCALE_BICUBIC : SCALE_DEFAULT;
}

static void scale_effect_missing(void **state)
{
	(void)state;
	/* each type, low resolution or not, with each effect missing */
	for (size_t t = 0; t < COUNT(every_type); t++) {
		int type = every_type[t];
		for (int lowres = 0; lowres < 2; lowres++) {
			uint32_t out_w = lowres ? 640 : 1280, out_h = lowres ? 360 : 720;
			for (int missing = SCALE_DEFAULT; missing <= SCALE_AREA; missing++) {
				uint32_t loaded = LOADED_ALL & ~OBS_VIDEO_SCALE_LOADED(missing);
				enum obs_video_scale_effect got =
					choose(1920, 1080, out_w, out_h, out_w, out_h, type, loaded);
				enum obs_video_scale_effect pick = lowres ? SCALE_LOWRES : type_effect(type);
				enum obs_video_scale_effect want = pick;
				if ((int)pick == missing)
					want = missing == SCALE_BICUBIC ? SCALE_DEFAULT : SCALE_BICUBIC;
				if (got != want) {
					fprintf(stderr, "type %d lowres %d missing %d: %d, expected %d\n", type, lowres,
						missing, (int)got, (int)want);
					fail();
				}
			}
		}
	}
	/* bilinear picks the default: without it, bicubic stands in */
	assert_int_equal(choose(1920, 1080, 1280, 720, 1280, 720, OBS_VIDEO_COPY_SCALE_BILINEAR, 0x1e), SCALE_BICUBIC);
	/* nothing loaded: the default */
	assert_int_equal(choose(1920, 1080, 1280, 720, 1280, 720, OBS_VIDEO_COPY_SCALE_AREA, 0), SCALE_DEFAULT);
	assert_int_equal(choose(1920, 1080, 640, 360, 640, 360, OBS_VIDEO_COPY_SCALE_AREA, 0), SCALE_DEFAULT);
	/* bits past the five effects do not matter */
	assert_int_equal(choose(1920, 1080, 1280, 720, 1280, 720, OBS_VIDEO_COPY_SCALE_AREA, 0xffffffe0u),
			 SCALE_DEFAULT);
}

static void scale_every_mask(void **state)
{
	(void)state;
	/* each type x low resolution or not x close or not x all 32 masks */
	for (size_t t = 0; t < COUNT(every_type); t++)
		for (int lowres = 0; lowres < 2; lowres++)
			for (int close = 0; close < 2; close++)
				for (uint32_t loaded = 0; loaded <= LOADED_ALL; loaded++) {
					uint32_t out_w = lowres ? 640 : 1280, out_h = lowres ? 360 : 720;
					uint32_t tw = close ? 1910 : out_w, th = close ? 1070 : out_h;
					enum obs_video_scale_effect got =
						choose(1920, 1080, tw, th, out_w, out_h, every_type[t], loaded);
					enum obs_video_scale_effect want =
						close ? SCALE_DEFAULT : expected(every_type[t], lowres, loaded);
					if (got != want) {
						fprintf(stderr,
							"type %d lowres %d close %d loaded %#x: %d, expected %d\n",
							every_type[t], lowres, close, loaded, (int)got, (int)want);
						fail();
					}
				}
}

/* ------------------------------------------------------------------------ */
/* obs_video_mix_reuse_match */

static struct obs_video_mix_reuse_key key(uintptr_t view, int space, uint32_t w, uint32_t h, bool rendered)
{
	struct obs_video_mix_reuse_key k = {view, space, w, h, rendered};
	return k;
}

static bool reuse(struct obs_video_mix_reuse_key other, struct obs_video_mix_reuse_key mix)
{
	return obs_video_mix_reuse_match(&other, &mix);
}

static void reuse_match(void **state)
{
	(void)state;
	const uintptr_t view = 0x1000;
	/* the same view, render space and base size, rendered */
	assert_true(reuse(key(view, 0, 1920, 1080, true), key(view, 0, 1920, 1080, false)));
	/* this mix's own rendered flag is not looked at */
	assert_true(reuse(key(view, 2, 1920, 1080, true), key(view, 2, 1920, 1080, true)));
	/* each one differing */
	assert_false(reuse(key(view, 0, 1920, 1080, false), key(view, 0, 1920, 1080, false)));
	assert_false(reuse(key(view + 1, 0, 1920, 1080, true), key(view, 0, 1920, 1080, false)));
	assert_false(reuse(key(view, 1, 1920, 1080, true), key(view, 0, 1920, 1080, false)));
	assert_false(reuse(key(view, 3, 1920, 1080, true), key(view, 2, 1920, 1080, false)));
	assert_false(reuse(key(view, 0, 1921, 1080, true), key(view, 0, 1920, 1080, false)));
	assert_false(reuse(key(view, 0, 1920, 1081, true), key(view, 0, 1920, 1080, false)));
	assert_false(reuse(key(view, 0, 1080, 1920, true), key(view, 0, 1920, 1080, false)));
	/* no view (0) matches no view; render spaces outside the enum compare
	 * as numbers */
	assert_true(reuse(key(0, 0, 0, 0, true), key(0, 0, 0, 0, false)));
	assert_true(reuse(key(UINTPTR_MAX, -1, UINT32_MAX, UINT32_MAX, true),
			  key(UINTPTR_MAX, -1, UINT32_MAX, UINT32_MAX, false)));
	assert_false(reuse(key(view, -1, 1920, 1080, true), key(view, 0, 1920, 1080, false)));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(gpu_formats_without_copy),
		cmocka_unit_test(gpu_every_format_has_its_copies),
		cmocka_unit_test(gpu_i420),
		cmocka_unit_test(gpu_nv12),
		cmocka_unit_test(gpu_nv12_packed),
		cmocka_unit_test(gpu_i444),
		cmocka_unit_test(gpu_i010),
		cmocka_unit_test(gpu_p010),
		cmocka_unit_test(gpu_p216_p416),
		cmocka_unit_test(gpu_each_line_size),
		cmocka_unit_test(gpu_widths_and_heights),
		cmocka_unit_test(gpu_width_wrap),
		cmocka_unit_test(rgbx_equal_line_sizes),
		cmocka_unit_test(rgbx_rows),
		cmocka_unit_test(resolution_close_edges),
		cmocka_unit_test(scale_close_is_default),
		cmocka_unit_test(scale_type_table),
		cmocka_unit_test(scale_lowres_rule),
		cmocka_unit_test(scale_effect_missing),
		cmocka_unit_test(scale_every_mask),
		cmocka_unit_test(reuse_match),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
