#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-source-frame-copy.h"
#include "media-io/video-io.h"

/*
 * Characterization of the async frame copy plan and cache decisions of
 * libobs/obs-source.c, moved to libobs/obs-source-frame-copy.c before their
 * Rust port:
 * - obs_source_frame_copy_plan(): what copy_frame_data() copies: the colour
 *   range when the frame is not full range, then for each plane of the
 *   format its rows ((height + 1) / 2 for 4:2:0 chroma), as one copy of
 *   line size x rows when the two line sizes are equal, else rows of the
 *   smaller line size;
 * - obs_source_frame_copy_cache_step(): cache_video()'s queue limit at
 *   MAX_ASYNC_FRAMES and async_texture_changed()'s reset;
 * - obs_source_frame_copy_cache_clean(): one entry of clean_cache(), the
 *   drop at MAX_UNUSED_FRAME_DURATION;
 * - obs_source_frame_copy_full_range() and _full_range2(): the range rule
 *   of obs_source_output_video(), _preload_video(), _set_video_frame() and
 *   their *2() variants;
 * - obs_source_frame_copy_preload_changed(): preload_frame_changed().
 * They pin the current C behaviour; cases marked "characterized, not
 * endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them, through their header. obs-source.c runs
 * the copies (every memcpy() stays there) and keeps the cache;
 * test_video_frame, test_async_frame_pacing_steps, the media restart tests,
 * the v4l2 and PipeWire camera tests, test_async_rotation_normalize and the
 * deinterlace tests cover the callers through the real libobs.
 *
 * Frames come from capture devices, decoded media and plugins, so the
 * format, height and line sizes can be anything: the plan's arithmetic is
 * the C's, in uint32_t and size_t.
 */

/* ------------------------------------------------------------------------ */
/* helpers */

enum rows_rule { ROWS_H, ROWS_HALF };

struct format_shape {
	int format;
	const char *name;
	size_t count;
	enum rows_rule rows[OBS_SOURCE_FRAME_COPY_PLANES];
};

/* Every enum video_format value in order, with the planes the copy makes. */
static const struct format_shape shapes[] = {
	{VIDEO_FORMAT_NONE, "NONE", 1, {ROWS_H}},
	{VIDEO_FORMAT_I420, "I420", 3, {ROWS_H, ROWS_HALF, ROWS_HALF}},
	{VIDEO_FORMAT_NV12, "NV12", 2, {ROWS_H, ROWS_HALF}},
	{VIDEO_FORMAT_YVYU, "YVYU", 1, {ROWS_H}},
	{VIDEO_FORMAT_YUY2, "YUY2", 1, {ROWS_H}},
	{VIDEO_FORMAT_UYVY, "UYVY", 1, {ROWS_H}},
	{VIDEO_FORMAT_RGBA, "RGBA", 1, {ROWS_H}},
	{VIDEO_FORMAT_BGRA, "BGRA", 1, {ROWS_H}},
	{VIDEO_FORMAT_BGRX, "BGRX", 1, {ROWS_H}},
	{VIDEO_FORMAT_Y800, "Y800", 1, {ROWS_H}},
	{VIDEO_FORMAT_I444, "I444", 3, {ROWS_H, ROWS_H, ROWS_H}},
	{VIDEO_FORMAT_BGR3, "BGR3", 1, {ROWS_H}},
	{VIDEO_FORMAT_I422, "I422", 3, {ROWS_H, ROWS_H, ROWS_H}},
	{VIDEO_FORMAT_I40A, "I40A", 4, {ROWS_H, ROWS_HALF, ROWS_HALF, ROWS_H}},
	{VIDEO_FORMAT_I42A, "I42A", 4, {ROWS_H, ROWS_H, ROWS_H, ROWS_H}},
	{VIDEO_FORMAT_YUVA, "YUVA", 4, {ROWS_H, ROWS_H, ROWS_H, ROWS_H}},
	{VIDEO_FORMAT_AYUV, "AYUV", 1, {ROWS_H}},
	{VIDEO_FORMAT_I010, "I010", 3, {ROWS_H, ROWS_HALF, ROWS_HALF}},
	{VIDEO_FORMAT_P010, "P010", 2, {ROWS_H, ROWS_HALF}},
	{VIDEO_FORMAT_I210, "I210", 3, {ROWS_H, ROWS_H, ROWS_H}},
	{VIDEO_FORMAT_I412, "I412", 3, {ROWS_H, ROWS_H, ROWS_H}},
	{VIDEO_FORMAT_YA2L, "YA2L", 4, {ROWS_H, ROWS_H, ROWS_H, ROWS_H}},
	/* "Unimplemented": characterized, not endorsed. No in-tree source
	 * outputs them. */
	{VIDEO_FORMAT_P216, "P216", 0, {ROWS_H}},
	{VIDEO_FORMAT_P416, "P416", 0, {ROWS_H}},
	{VIDEO_FORMAT_V210, "V210", 1, {ROWS_H}},
	{VIDEO_FORMAT_R10L, "R10L", 1, {ROWS_H}},
};

#define SHAPE_COUNT (sizeof(shapes) / sizeof(shapes[0]))

/* YUV per format_is_yuv(), in enum order */
static const bool yuv[] = {
	false, true, true, true, true, true, false, false, false, false, true, false, true,
	true,  true, true, true, true, true, true,  true,  true,  true,  true, true,  false,
};

static const uint32_t heights[] = {0, 1, 2, 3, 4, 5, 9, 719, 720, 1079, 1080, 1081};

#define HEIGHT_COUNT (sizeof(heights) / sizeof(heights[0]))

static uint32_t rows_of(enum rows_rule rule, uint32_t height)
{
	return rule == ROWS_HALF ? (height + 1) / 2 : height;
}

static struct obs_source_frame_copy_lines lines4(uint32_t l0, uint32_t l1, uint32_t l2, uint32_t l3)
{
	struct obs_source_frame_copy_lines lines = {{l0, l1, l2, l3}};
	return lines;
}

static struct obs_source_frame_copy_lines lines_all(uint32_t l)
{
	return lines4(l, l, l, l);
}

static void check_copy(const char *what, size_t plane, const struct obs_source_frame_plane_copy *got, bool whole,
		       size_t bytes, uint32_t rows, uint32_t src_stride, uint32_t dst_stride)
{
	if (got->whole != whole || got->bytes != bytes || got->rows != rows || got->src_stride != src_stride ||
	    got->dst_stride != dst_stride) {
		fprintf(stderr,
			"%s plane %zu: whole %d bytes %zu rows %u strides %u/%u, "
			"expected whole %d bytes %zu rows %u strides %u/%u\n",
			what, plane, got->whole, got->bytes, got->rows, got->src_stride, got->dst_stride, whole, bytes,
			rows, src_stride, dst_stride);
		fail();
	}
}

static void check_count(const char *what, const struct obs_source_frame_copy_plan *plan, size_t count)
{
	if (plan->count != count) {
		fprintf(stderr, "%s: %zu planes, expected %zu\n", what, plan->count, count);
		fail();
	}
}

/* The copy plane i makes for a source line size s and a destination line
 * size d over rows rows, as the C decides it. */
static void check_relation(const char *what, size_t plane, const struct obs_source_frame_plane_copy *got, uint32_t s,
			   uint32_t d, uint32_t rows)
{
	if (s == d)
		check_copy(what, plane, got, true, (size_t)d * (size_t)rows, rows, s, d);
	else
		check_copy(what, plane, got, false, d < s ? d : s, rows, s, d);
}

static const struct obs_source_frame_plane_copy zero_copy = {0};

static void check_unused_zero(const char *what, const struct obs_source_frame_copy_plan *plan)
{
	for (size_t i = plan->count; i < OBS_SOURCE_FRAME_COPY_PLANES; i++)
		check_copy(what, i, &plan->planes[i], zero_copy.whole, zero_copy.bytes, zero_copy.rows,
			   zero_copy.src_stride, zero_copy.dst_stride);
}

static struct obs_source_frame_copy_key key(uint32_t width, uint32_t height, int convert_type)
{
	struct obs_source_frame_copy_key k = {width, height, convert_type};
	return k;
}

static struct obs_source_frame_copy_shape shape(int format, uint32_t width, uint32_t height)
{
	struct obs_source_frame_copy_shape s = {format, width, height};
	return s;
}

static void check_age(const char *what, struct obs_source_frame_cache_age got, bool counted, long unused_count,
		      bool drop)
{
	if (got.counted != counted || got.unused_count != unused_count || got.drop != drop) {
		fprintf(stderr, "%s: counted %d unused %ld drop %d, expected counted %d unused %ld drop %d\n", what,
			got.counted, got.unused_count, got.drop, counted, unused_count, drop);
		fail();
	}
}

/* ------------------------------------------------------------------------ */
/* constants */

static void constants(void **state)
{
	(void)state;
	assert_int_equal(OBS_SOURCE_FRAME_COPY_PLANES, 4);
	assert_int_equal(OBS_SOURCE_FRAME_COPY_MAX_ASYNC_FRAMES, 30);
	assert_int_equal(OBS_SOURCE_FRAME_COPY_MAX_UNUSED, 5);
	assert_int_equal(OBS_SOURCE_FRAME_CACHE_KEEP, 0);
	assert_int_equal(OBS_SOURCE_FRAME_CACHE_RESET, 1);
	assert_int_equal(OBS_SOURCE_FRAME_CACHE_OVERFLOW, 2);
	assert_true(OBS_SOURCE_FRAME_COPY_PLANES <= MAX_AV_PLANES);
}

/* The shape table lists every enum video_format value once, in order. */
static void table_covers_every_format(void **state)
{
	(void)state;
	assert_int_equal(SHAPE_COUNT, VIDEO_FORMAT_R10L + 1);
	assert_int_equal(sizeof(yuv) / sizeof(yuv[0]), VIDEO_FORMAT_R10L + 1);
	for (size_t i = 0; i < SHAPE_COUNT; i++)
		assert_int_equal(shapes[i].format, (int)i);
}

/* ------------------------------------------------------------------------ */
/* the copy plan */

/* Every format at every height, equal line sizes: one copy of line size x
 * rows per plane. */
static void plan_every_format_equal_lines(void **state)
{
	(void)state;
	for (size_t f = 0; f < SHAPE_COUNT; f++) {
		for (size_t h = 0; h < HEIGHT_COUNT; h++) {
			const struct obs_source_frame_copy_lines l = lines4(1920, 960, 961, 1921);
			struct obs_source_frame_copy_plan plan =
				obs_source_frame_copy_plan(shapes[f].format, true, heights[h], l, l);
			check_count(shapes[f].name, &plan, shapes[f].count);
			for (size_t i = 0; i < plan.count; i++) {
				const uint32_t rows = rows_of(shapes[f].rows[i], heights[h]);
				check_copy(shapes[f].name, i, &plan.planes[i], true, (size_t)l.linesize[i] * rows, rows,
					   l.linesize[i], l.linesize[i]);
			}
			check_unused_zero(shapes[f].name, &plan);
		}
	}
}

/* Every format at every height, the source's line sizes padded: rows of the
 * destination's line size. */
static void plan_every_format_padded_source(void **state)
{
	(void)state;
	for (size_t f = 0; f < SHAPE_COUNT; f++) {
		for (size_t h = 0; h < HEIGHT_COUNT; h++) {
			const struct obs_source_frame_copy_lines src = lines4(2048, 1024, 1024, 2048);
			const struct obs_source_frame_copy_lines dst = lines4(1920, 960, 960, 1920);
			struct obs_source_frame_copy_plan plan =
				obs_source_frame_copy_plan(shapes[f].format, true, heights[h], src, dst);
			check_count(shapes[f].name, &plan, shapes[f].count);
			for (size_t i = 0; i < plan.count; i++) {
				const uint32_t rows = rows_of(shapes[f].rows[i], heights[h]);
				check_copy(shapes[f].name, i, &plan.planes[i], false, dst.linesize[i], rows,
					   src.linesize[i], dst.linesize[i]);
			}
		}
	}
}

/* Values outside the enum copy no plane, nor do P216 and P416
 * (characterized, not endorsed). */
static void plan_formats_without_copy(void **state)
{
	(void)state;
	const int formats[] = {-1, VIDEO_FORMAT_R10L + 1, 1000, INT_MIN, INT_MAX, VIDEO_FORMAT_P216, VIDEO_FORMAT_P416};
	for (size_t i = 0; i < sizeof(formats) / sizeof(formats[0]); i++) {
		for (size_t h = 0; h < HEIGHT_COUNT; h++) {
			struct obs_source_frame_copy_plan plan =
				obs_source_frame_copy_plan(formats[i], false, heights[h], lines_all(64), lines_all(32));
			check_count("no copy", &plan, 0);
			check_unused_zero("no copy", &plan);
			/* the colour range is still copied */
			assert_true(plan.color_range);
		}
	}
}

/* 4:2:0 chroma rows round up: odd heights get the last half row. */
static void plan_odd_heights(void **state)
{
	(void)state;
	const int formats[] = {VIDEO_FORMAT_I420, VIDEO_FORMAT_I010, VIDEO_FORMAT_NV12, VIDEO_FORMAT_P010,
			       VIDEO_FORMAT_I40A};
	const uint32_t h[] = {1, 3, 5, 719, 1081, 2161};
	const uint32_t half[] = {1, 2, 3, 360, 541, 1081};
	for (size_t f = 0; f < sizeof(formats) / sizeof(formats[0]); f++) {
		for (size_t i = 0; i < sizeof(h) / sizeof(h[0]); i++) {
			struct obs_source_frame_copy_plan plan =
				obs_source_frame_copy_plan(formats[f], true, h[i], lines_all(16), lines_all(16));
			assert_int_equal(plan.planes[0].rows, h[i]);
			assert_int_equal(plan.planes[1].rows, half[i]);
			if (plan.count > 2)
				assert_int_equal(plan.planes[2].rows, half[i]);
			if (plan.count > 3)
				assert_int_equal(plan.planes[3].rows, h[i]);
		}
	}
	/* 4:2:2 and 4:4:4 chroma keep the full height */
	struct obs_source_frame_copy_plan plan =
		obs_source_frame_copy_plan(VIDEO_FORMAT_I422, true, 1081, lines_all(16), lines_all(16));
	assert_int_equal(plan.planes[1].rows, 1081);
	assert_int_equal(plan.planes[2].rows, 1081);
}

/* (height + 1) / 2 wraps in uint32_t: UINT32_MAX has 0 chroma rows
 * (characterized, not endorsed). */
static void plan_height_wrap(void **state)
{
	(void)state;
	struct obs_source_frame_copy_plan plan =
		obs_source_frame_copy_plan(VIDEO_FORMAT_I420, true, UINT32_MAX, lines_all(8), lines_all(8));
	check_count("I420 UINT32_MAX", &plan, 3);
	check_copy("I420 UINT32_MAX", 0, &plan.planes[0], true, (size_t)8 * UINT32_MAX, UINT32_MAX, 8, 8);
	check_copy("I420 UINT32_MAX", 1, &plan.planes[1], true, 0, 0, 8, 8);
	check_copy("I420 UINT32_MAX", 2, &plan.planes[2], true, 0, 0, 8, 8);

	plan = obs_source_frame_copy_plan(VIDEO_FORMAT_NV12, true, UINT32_MAX - 1, lines_all(8), lines_all(9));
	check_copy("NV12 UINT32_MAX - 1", 1, &plan.planes[1], false, 8, 0x7fffffffu, 8, 9);
}

/* The one copy's size is line size x rows in size_t (64 bits here; it
 * would wrap with a 32-bit size_t). */
static void plan_whole_size(void **state)
{
	(void)state;
	struct obs_source_frame_copy_plan plan = obs_source_frame_copy_plan(
		VIDEO_FORMAT_RGBA, true, UINT32_MAX, lines_all(UINT32_MAX), lines_all(UINT32_MAX));
	check_copy("RGBA max", 0, &plan.planes[0], true, (size_t)UINT32_MAX * (size_t)UINT32_MAX, UINT32_MAX,
		   UINT32_MAX, UINT32_MAX);
}

/* Each plane decides on its own two line sizes: equal, a smaller, larger or
 * 0 source, a 0 destination, both 0. */
static void plan_each_line_size(void **state)
{
	(void)state;
	/* source line size relative to the destination's 100 */
	const uint32_t rel[] = {100, 99, 1, 101, UINT32_MAX, 0};
	for (size_t f = 0; f < SHAPE_COUNT; f++) {
		for (size_t p = 0; p < shapes[f].count; p++) {
			for (size_t r = 0; r < sizeof(rel) / sizeof(rel[0]); r++) {
				struct obs_source_frame_copy_lines src = lines_all(100);
				const struct obs_source_frame_copy_lines dst = lines_all(100);
				src.linesize[p] = rel[r];
				struct obs_source_frame_copy_plan plan =
					obs_source_frame_copy_plan(shapes[f].format, true, 7, src, dst);
				check_count(shapes[f].name, &plan, shapes[f].count);
				for (size_t i = 0; i < plan.count; i++)
					check_relation(shapes[f].name, i, &plan.planes[i], src.linesize[i],
						       dst.linesize[i], rows_of(shapes[f].rows[i], 7));
			}
			/* a 0 destination line size: rows of 0 bytes */
			struct obs_source_frame_copy_lines dst = lines_all(100);
			dst.linesize[p] = 0;
			struct obs_source_frame_copy_plan plan =
				obs_source_frame_copy_plan(shapes[f].format, true, 7, lines_all(100), dst);
			check_copy(shapes[f].name, p, &plan.planes[p], false, 0, rows_of(shapes[f].rows[p], 7), 100, 0);
			/* both 0: one copy of 0 bytes */
			plan = obs_source_frame_copy_plan(shapes[f].format, true, 7, lines_all(0), lines_all(0));
			check_copy(shapes[f].name, p, &plan.planes[p], true, 0, rows_of(shapes[f].rows[p], 7), 0, 0);
		}
	}
}

/* Only the first four line sizes are planes; a fifth is never read. */
static void plan_line_sizes_beyond_planes(void **state)
{
	(void)state;
	struct obs_source_frame_copy_plan plan =
		obs_source_frame_copy_plan(VIDEO_FORMAT_YUVA, true, 2, lines4(1, 2, 3, 4), lines4(1, 2, 3, 5));
	check_count("YUVA", &plan, 4);
	check_copy("YUVA", 3, &plan.planes[3], false, 4, 2, 4, 5);
}

/* The colour range is copied only for a frame that is not full range, for
 * every format and outside the enum. */
static void plan_color_range(void **state)
{
	(void)state;
	for (int f = -2; f <= VIDEO_FORMAT_R10L + 2; f++) {
		struct obs_source_frame_copy_plan plan =
			obs_source_frame_copy_plan(f, false, 4, lines_all(4), lines_all(4));
		assert_true(plan.color_range);
		plan = obs_source_frame_copy_plan(f, true, 4, lines_all(4), lines_all(4));
		assert_false(plan.color_range);
	}
}

/* ------------------------------------------------------------------------ */
/* the cache */

/* The queue limit comes first: at MAX_ASYNC_FRAMES or more queued frames,
 * the cache is cleared and the frame refused, whatever the keys. */
static void cache_queue_limit(void **state)
{
	(void)state;
	const struct obs_source_frame_copy_key same = key(1920, 1080, 2);
	const struct obs_source_frame_copy_key other = key(1280, 720, 1);
	const size_t limit = OBS_SOURCE_FRAME_COPY_MAX_ASYNC_FRAMES;

	assert_int_equal(obs_source_frame_copy_cache_step(0, same, same), OBS_SOURCE_FRAME_CACHE_KEEP);
	assert_int_equal(obs_source_frame_copy_cache_step(limit - 1, same, same), OBS_SOURCE_FRAME_CACHE_KEEP);
	assert_int_equal(obs_source_frame_copy_cache_step(limit - 1, same, other), OBS_SOURCE_FRAME_CACHE_RESET);
	assert_int_equal(obs_source_frame_copy_cache_step(limit, same, same), OBS_SOURCE_FRAME_CACHE_OVERFLOW);
	assert_int_equal(obs_source_frame_copy_cache_step(limit, same, other), OBS_SOURCE_FRAME_CACHE_OVERFLOW);
	assert_int_equal(obs_source_frame_copy_cache_step(limit + 1, same, same), OBS_SOURCE_FRAME_CACHE_OVERFLOW);
	assert_int_equal(obs_source_frame_copy_cache_step(limit + 1, same, other), OBS_SOURCE_FRAME_CACHE_OVERFLOW);
	assert_int_equal(obs_source_frame_copy_cache_step(SIZE_MAX, same, same), OBS_SOURCE_FRAME_CACHE_OVERFLOW);
}

/* async_texture_changed(): a change in the width, the height or the
 * convert type alone resets the cache. */
static void cache_texture_changed(void **state)
{
	(void)state;
	const struct obs_source_frame_copy_key cache = key(1920, 1080, 2);
	assert_int_equal(obs_source_frame_copy_cache_step(3, cache, key(1920, 1080, 2)), OBS_SOURCE_FRAME_CACHE_KEEP);
	assert_int_equal(obs_source_frame_copy_cache_step(3, cache, key(1921, 1080, 2)), OBS_SOURCE_FRAME_CACHE_RESET);
	assert_int_equal(obs_source_frame_copy_cache_step(3, cache, key(1920, 1079, 2)), OBS_SOURCE_FRAME_CACHE_RESET);
	assert_int_equal(obs_source_frame_copy_cache_step(3, cache, key(1920, 1080, 3)), OBS_SOURCE_FRAME_CACHE_RESET);
	assert_int_equal(obs_source_frame_copy_cache_step(3, cache, key(1080, 1920, 2)), OBS_SOURCE_FRAME_CACHE_RESET);
	/* a new source's cache is 0 x 0 and the convert type of format 0 */
	assert_int_equal(obs_source_frame_copy_cache_step(0, key(0, 0, 0), key(0, 0, 0)), OBS_SOURCE_FRAME_CACHE_KEEP);
	assert_int_equal(obs_source_frame_copy_cache_step(0, key(UINT32_MAX, UINT32_MAX, INT_MIN),
							  key(UINT32_MAX, UINT32_MAX, INT_MIN)),
			 OBS_SOURCE_FRAME_CACHE_KEEP);
	assert_int_equal(obs_source_frame_copy_cache_step(0, key(1, 1, INT_MAX), key(1, 1, INT_MIN)),
			 OBS_SOURCE_FRAME_CACHE_RESET);
}

/* clean_cache(): an unused entry counts one more call, and is dropped when
 * the count reaches MAX_UNUSED_FRAME_DURATION. */
static void cache_clean_counts(void **state)
{
	(void)state;
	check_age("unused 0", obs_source_frame_copy_cache_clean(false, 0), true, 1, false);
	check_age("unused 1", obs_source_frame_copy_cache_clean(false, 1), true, 2, false);
	check_age("unused 2", obs_source_frame_copy_cache_clean(false, 2), true, 3, false);
	check_age("unused 3", obs_source_frame_copy_cache_clean(false, 3), true, 4, false);
	check_age("unused 4", obs_source_frame_copy_cache_clean(false, 4), true, 5, true);
	/* past the limit it is never dropped: unreachable, as the entry is
	 * erased at the limit (characterized, not endorsed) */
	check_age("unused 5", obs_source_frame_copy_cache_clean(false, 5), true, 6, false);
	check_age("unused 6", obs_source_frame_copy_cache_clean(false, 6), true, 7, false);
	check_age("unused -1", obs_source_frame_copy_cache_clean(false, -1), true, 0, false);
	check_age("unused LONG_MIN", obs_source_frame_copy_cache_clean(false, LONG_MIN), true, LONG_MIN + 1, false);
	check_age("unused LONG_MAX - 1", obs_source_frame_copy_cache_clean(false, LONG_MAX - 1), true, LONG_MAX, false);
}

/* A used entry is not counted and keeps its count, whatever it is. */
static void cache_clean_used(void **state)
{
	(void)state;
	const long counts[] = {0, 1, 3, 4, 5, 6, -1, LONG_MIN, LONG_MAX};
	for (size_t i = 0; i < sizeof(counts) / sizeof(counts[0]); i++)
		check_age("used", obs_source_frame_copy_cache_clean(true, counts[i]), false, counts[i], false);
}

/* The drop comes after exactly MAX_UNUSED_FRAME_DURATION calls from 0. */
static void cache_clean_drop_at_limit(void **state)
{
	(void)state;
	long count = 0;
	for (int call = 1; call <= OBS_SOURCE_FRAME_COPY_MAX_UNUSED; call++) {
		struct obs_source_frame_cache_age age = obs_source_frame_copy_cache_clean(false, count);
		assert_true(age.counted);
		assert_int_equal(age.unused_count, call);
		assert_int_equal(age.drop, call == OBS_SOURCE_FRAME_COPY_MAX_UNUSED);
		count = age.unused_count;
	}
}

/* ------------------------------------------------------------------------ */
/* the range rule */

/* obs_source_output_video() and its kin: a YUV frame keeps its range, any
 * other format (RGB, Y800, NONE, values outside the enum) is full range. */
static void full_range_rule(void **state)
{
	(void)state;
	for (int f = 0; f <= VIDEO_FORMAT_R10L; f++) {
		assert_int_equal(obs_source_frame_copy_full_range(f, false), !yuv[f]);
		assert_true(obs_source_frame_copy_full_range(f, true));
	}
	const int outside[] = {-1, VIDEO_FORMAT_R10L + 1, 1000, INT_MIN, INT_MAX};
	for (size_t i = 0; i < sizeof(outside) / sizeof(outside[0]); i++) {
		assert_true(obs_source_frame_copy_full_range(outside[i], false));
		assert_true(obs_source_frame_copy_full_range(outside[i], true));
	}
}

/* The *2() variants: full range exactly when resolve_video_range() gives
 * VIDEO_RANGE_FULL. VIDEO_RANGE_DEFAULT is partial for YUV and full for any
 * other format; an explicit range is kept, so an RGB frame with
 * VIDEO_RANGE_PARTIAL stays limited here, unlike the rule above. A range
 * outside the enum is not full. */
static void full_range2_rule(void **state)
{
	(void)state;
	for (int f = 0; f <= VIDEO_FORMAT_R10L; f++) {
		assert_int_equal(obs_source_frame_copy_full_range2(f, VIDEO_RANGE_DEFAULT), !yuv[f]);
		assert_false(obs_source_frame_copy_full_range2(f, VIDEO_RANGE_PARTIAL));
		assert_true(obs_source_frame_copy_full_range2(f, VIDEO_RANGE_FULL));
		assert_false(obs_source_frame_copy_full_range2(f, VIDEO_RANGE_FULL + 1));
		assert_false(obs_source_frame_copy_full_range2(f, -1));
		assert_false(obs_source_frame_copy_full_range2(f, INT_MIN));
		assert_false(obs_source_frame_copy_full_range2(f, INT_MAX));
	}
	const int outside[] = {-1, VIDEO_FORMAT_R10L + 1, INT_MIN, INT_MAX};
	for (size_t i = 0; i < sizeof(outside) / sizeof(outside[0]); i++) {
		assert_true(obs_source_frame_copy_full_range2(outside[i], VIDEO_RANGE_DEFAULT));
		assert_false(obs_source_frame_copy_full_range2(outside[i], VIDEO_RANGE_PARTIAL));
		assert_true(obs_source_frame_copy_full_range2(outside[i], VIDEO_RANGE_FULL));
	}
}

/* ------------------------------------------------------------------------ */
/* the preload frame */

static void preload_changed(void **state)
{
	(void)state;
	const struct obs_source_frame_copy_shape p = shape(VIDEO_FORMAT_NV12, 1920, 1080);
	/* no preload frame: always a new one, whatever the shapes */
	assert_true(obs_source_frame_copy_preload_changed(false, p, p));
	assert_true(obs_source_frame_copy_preload_changed(false, shape(0, 0, 0), shape(0, 0, 0)));
	assert_false(obs_source_frame_copy_preload_changed(true, p, p));
	assert_true(obs_source_frame_copy_preload_changed(true, p, shape(VIDEO_FORMAT_NV12, 1921, 1080)));
	assert_true(obs_source_frame_copy_preload_changed(true, p, shape(VIDEO_FORMAT_NV12, 1920, 1081)));
	assert_true(obs_source_frame_copy_preload_changed(true, p, shape(VIDEO_FORMAT_I420, 1920, 1080)));
	assert_true(obs_source_frame_copy_preload_changed(true, p, shape(VIDEO_FORMAT_NV12, 1080, 1920)));
	/* the format alone, not its convert type or range: P010 and I010 are
	 * both 4:2:0 */
	assert_true(obs_source_frame_copy_preload_changed(true, shape(VIDEO_FORMAT_P010, 64, 64),
							  shape(VIDEO_FORMAT_I010, 64, 64)));
	assert_false(obs_source_frame_copy_preload_changed(true, shape(INT_MIN, UINT32_MAX, 0),
							   shape(INT_MIN, UINT32_MAX, 0)));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(constants),
		cmocka_unit_test(table_covers_every_format),
		cmocka_unit_test(plan_every_format_equal_lines),
		cmocka_unit_test(plan_every_format_padded_source),
		cmocka_unit_test(plan_formats_without_copy),
		cmocka_unit_test(plan_odd_heights),
		cmocka_unit_test(plan_height_wrap),
		cmocka_unit_test(plan_whole_size),
		cmocka_unit_test(plan_each_line_size),
		cmocka_unit_test(plan_line_sizes_beyond_planes),
		cmocka_unit_test(plan_color_range),
		cmocka_unit_test(cache_queue_limit),
		cmocka_unit_test(cache_texture_changed),
		cmocka_unit_test(cache_clean_counts),
		cmocka_unit_test(cache_clean_used),
		cmocka_unit_test(cache_clean_drop_at_limit),
		cmocka_unit_test(full_range_rule),
		cmocka_unit_test(full_range2_rule),
		cmocka_unit_test(preload_changed),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
