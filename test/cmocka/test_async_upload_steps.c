#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <inttypes.h>
#include <limits.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs.h"
#include "obs-source-conversion-plan.h"
#include "obs-source-async-upload.h"

/*
 * Characterization of the async video upload decisions of
 * libobs/obs-source.c, moved to libobs/obs-source-async-upload.c before its
 * Rust port:
 * - obs_source_async_upload_texture_changed() and
 *   obs_source_async_upload_tries_gpu(): set_async_texture_size()'s compare
 *   of the five fields that rebuilds the textures, and whether the rebuild
 *   tries a GPU conversion;
 * - obs_source_async_upload_size_uniforms(), _hlg_peak(), _maximum_nits(),
 *   _over_sdr_white(), _hdr_lw() and _sets_range(): the shader parameters
 *   of update_async_texrender(), as float bits;
 * - obs_source_async_upload_linear_alpha(), _uses_texrender() and _plain():
 *   update_async_textures()'s linear alpha flag and its route (texrender,
 *   one plain texture, or neither);
 * - obs_source_async_upload_bgr_swap(): check_to_swap_bgrx_bgra();
 * - obs_source_async_upload_reanchor_reads_unbuffered() and
 *   obs_source_async_upload_reanchor(): the timing re-anchor of
 *   obs_source_update_async_video().
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_async_conversion_plan,
 * test_pipewire_cursor_hdr, test_media_format_change,
 * test_pipewire_frame_layout, test_v4l2_corrupt_frame,
 * test_video_output_formats and test_filter_color_space cover the callers
 * through the real libobs.
 */

static uint32_t float_bits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

static float bits_float(uint32_t u)
{
	float f;
	memcpy(&f, &u, sizeof(f));
	return f;
}

static void check_int(long long actual, long long expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %lld, expected %lld\n", line, what, actual, expected);
		fail();
	}
}

static void check_bool(bool actual, bool expected, const char *what, int line)
{
	if (actual != expected) {
		fprintf(stderr, "line %d: %s is %s, expected %s\n", line, what, actual ? "true" : "false",
			expected ? "true" : "false");
		fail();
	}
}

static void check_bits(float actual, uint32_t expected, const char *what, int line)
{
	if (float_bits(actual) != expected) {
		fprintf(stderr, "line %d: %s is %a (0x%08" PRIx32 "), expected %a (0x%08" PRIx32 ")\n", line, what,
			(double)actual, float_bits(actual), (double)bits_float(expected), expected);
		fail();
	}
}

#define CHECK_INT(actual, expected) check_int((long long)(actual), (long long)(expected), #actual, __LINE__)
#define CHECK_BOOL(actual, expected) check_bool((actual), (expected), #actual, __LINE__)
#define CHECK_BITS(actual, expected) check_bits((actual), (expected), #actual, __LINE__)

#define F_NAN 0x7fc00000u
#define F_INF 0x7f800000u
#define F_NEG_INF 0xff800000u

/* --- enum values and layouts ------------------------------------------- */

/* The Rust port uses these numbers and the two structs' layouts. */
static void enum_values_and_layouts(void **state)
{
	(void)state;
	CHECK_INT(GS_UNKNOWN, 0);
	CHECK_INT(GS_BGRX, 4);
	CHECK_INT(GS_BGRA, 5);
	CHECK_INT(GS_BGRX_UNORM, 20);
	CHECK_INT(GS_BGRA_UNORM, 21);
	CHECK_INT(VIDEO_FORMAT_NONE, 0);
	CHECK_INT(VIDEO_FORMAT_RGBA, 6);
	CHECK_INT(VIDEO_FORMAT_BGRA, 7);
	CHECK_INT(VIDEO_FORMAT_BGRX, 8);
	CHECK_INT(VIDEO_FORMAT_R10L, 25);
	CHECK_INT(VIDEO_TRC_DEFAULT, 0);
	CHECK_INT(VIDEO_TRC_SRGB, 1);
	CHECK_INT(VIDEO_TRC_PQ, 2);
	CHECK_INT(VIDEO_TRC_HLG, 3);
	CHECK_INT(OBS_SOURCE_FRAME_LINEAR_ALPHA, 1);
	CHECK_INT(OBS_SOURCE_CONVERT_NONE, 0);
	CHECK_INT(OBS_SOURCE_CONVERT_R10L, 20);
	CHECK_INT(sizeof(enum gs_color_format), sizeof(int));
	CHECK_INT(sizeof(enum video_format), sizeof(int));

	CHECK_INT(sizeof(struct obs_source_async_upload_format), 16);
	CHECK_INT(_Alignof(struct obs_source_async_upload_format), 4);
	CHECK_INT(offsetof(struct obs_source_async_upload_format, width), 0);
	CHECK_INT(offsetof(struct obs_source_async_upload_format, height), 4);
	CHECK_INT(offsetof(struct obs_source_async_upload_format, format), 8);
	CHECK_INT(offsetof(struct obs_source_async_upload_format, full_range), 12);
	CHECK_INT(offsetof(struct obs_source_async_upload_format, trc), 13);

	CHECK_INT(sizeof(struct obs_source_async_upload_size), 24);
	CHECK_INT(_Alignof(struct obs_source_async_upload_size), 4);
	CHECK_INT(offsetof(struct obs_source_async_upload_size, width), 0);
	CHECK_INT(offsetof(struct obs_source_async_upload_size, height), 4);
	CHECK_INT(offsetof(struct obs_source_async_upload_size, width_d2), 8);
	CHECK_INT(offsetof(struct obs_source_async_upload_size, height_d2), 12);
	CHECK_INT(offsetof(struct obs_source_async_upload_size, width_x2_i), 16);
	CHECK_INT(offsetof(struct obs_source_async_upload_size, height_x2_i), 20);
}

/* --- set_async_texture_size() ------------------------------------------ */

static struct obs_source_async_upload_format fmt(uint32_t width, uint32_t height, int format, bool full_range,
						 uint8_t trc)
{
	struct obs_source_async_upload_format f;
	memset(&f, 0, sizeof(f));
	f.width = width;
	f.height = height;
	f.format = format;
	f.full_range = full_range;
	f.trc = trc;
	return f;
}

/* The same five values keep the textures; each one changed alone rebuilds
 * them, in both directions. */
static void texture_changed_each_field(void **state)
{
	(void)state;
	const struct obs_source_async_upload_format base = fmt(1920, 1080, VIDEO_FORMAT_NV12, false, VIDEO_TRC_SRGB);

	CHECK_BOOL(obs_source_async_upload_texture_changed(base, base), false);

	struct obs_source_async_upload_format f = base;
	f.width = 1919;
	CHECK_BOOL(obs_source_async_upload_texture_changed(base, f), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(f, base), true);

	f = base;
	f.height = 1081;
	CHECK_BOOL(obs_source_async_upload_texture_changed(base, f), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(f, base), true);

	f = base;
	f.format = VIDEO_FORMAT_I420;
	CHECK_BOOL(obs_source_async_upload_texture_changed(base, f), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(f, base), true);

	f = base;
	f.full_range = true;
	CHECK_BOOL(obs_source_async_upload_texture_changed(base, f), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(f, base), true);

	f = base;
	f.trc = VIDEO_TRC_PQ;
	CHECK_BOOL(obs_source_async_upload_texture_changed(base, f), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(f, base), true);

	/* all five at once */
	f = fmt(1280, 720, VIDEO_FORMAT_BGRA, true, VIDEO_TRC_HLG);
	CHECK_BOOL(obs_source_async_upload_texture_changed(base, f), true);
}

/* A source's first frame: the textures' values start at 0 (VIDEO_FORMAT_NONE,
 * limited range, VIDEO_TRC_DEFAULT), so a 0 x 0 frame of that kind keeps
 * them (characterized, not endorsed: no texture is ever made for it). */
static void texture_changed_first_frame(void **state)
{
	(void)state;
	const struct obs_source_async_upload_format zero = fmt(0, 0, VIDEO_FORMAT_NONE, false, VIDEO_TRC_DEFAULT);

	CHECK_BOOL(obs_source_async_upload_texture_changed(zero, zero), false);
	CHECK_BOOL(obs_source_async_upload_texture_changed(zero, fmt(1, 0, VIDEO_FORMAT_NONE, false, 0)), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(zero, fmt(0, 1, VIDEO_FORMAT_NONE, false, 0)), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(zero, fmt(0, 0, VIDEO_FORMAT_I420, false, 0)), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(zero, fmt(0, 0, VIDEO_FORMAT_NONE, true, 0)), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(zero, fmt(0, 0, VIDEO_FORMAT_NONE, false, 1)), true);
}

/* The edges of each field compare as values: the largest sizes, formats and
 * transfers outside their enums. */
static void texture_changed_edges(void **state)
{
	(void)state;
	const struct obs_source_async_upload_format big = fmt(UINT32_MAX, UINT32_MAX, INT_MAX, true, 255);

	CHECK_BOOL(obs_source_async_upload_texture_changed(big, big), false);
	CHECK_BOOL(obs_source_async_upload_texture_changed(big, fmt(UINT32_MAX - 1, UINT32_MAX, INT_MAX, true, 255)),
		   true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(big, fmt(UINT32_MAX, 0, INT_MAX, true, 255)), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(big, fmt(UINT32_MAX, UINT32_MAX, INT_MIN, true, 255)), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(big, fmt(UINT32_MAX, UINT32_MAX, INT_MAX, true, 254)), true);
	CHECK_BOOL(obs_source_async_upload_texture_changed(fmt(1, 1, -1, false, 4), fmt(1, 1, -1, false, 4)), false);
	CHECK_BOOL(obs_source_async_upload_texture_changed(fmt(1, 1, -1, false, 4), fmt(1, 1, 26, false, 4)), true);
}

/* Every convert type but OBS_SOURCE_CONVERT_NONE tries a GPU conversion; the
 * plain upload is the other way round. Values outside the enum try one too. */
static void convert_type_routes(void **state)
{
	(void)state;
	CHECK_BOOL(obs_source_async_upload_tries_gpu(OBS_SOURCE_CONVERT_NONE), false);
	CHECK_BOOL(obs_source_async_upload_plain(OBS_SOURCE_CONVERT_NONE), true);
	for (int type = OBS_SOURCE_CONVERT_NV12; type <= OBS_SOURCE_CONVERT_R10L; type++) {
		CHECK_BOOL(obs_source_async_upload_tries_gpu(type), true);
		CHECK_BOOL(obs_source_async_upload_plain(type), false);
	}
	const int outside[] = {-1, 21, INT_MIN, INT_MAX};
	for (size_t i = 0; i < sizeof(outside) / sizeof(outside[0]); i++) {
		CHECK_BOOL(obs_source_async_upload_tries_gpu(outside[i]), true);
		CHECK_BOOL(obs_source_async_upload_plain(outside[i]), false);
	}
}

/* --- update_async_texrender() ------------------------------------------ */

struct size_case {
	uint32_t cx, cy;
	uint32_t width, height, width_d2, height_d2, width_x2_i, height_x2_i;
};

/* The size uniforms as float bits, each axis on its own. Characterized, not
 * endorsed: a size of 0 gives an infinite half texel, and sizes past 2^24
 * round (16777217 is 16777216, UINT32_MAX is 4294967296). */
static const struct size_case SIZES[] = {
	{0, 4294967295u, 0x00000000, 0x4f800000, 0x00000000, 0x4f000000, F_INF, 0x2f000000},
	{1, 16777217, 0x3f800000, 0x4b800000, 0x3f000000, 0x4b000000, 0x3f000000, 0x33000000},
	{2, 1920, 0x40000000, 0x44f00000, 0x3f800000, 0x44700000, 0x3e800000, 0x39888889},
	{3, 1919, 0x40400000, 0x44efe000, 0x3fc00000, 0x446fe000, 0x3e2aaaab, 0x39889abf},
	{1919, 3, 0x44efe000, 0x40400000, 0x446fe000, 0x3fc00000, 0x39889abf, 0x3e2aaaab},
	{1920, 2, 0x44f00000, 0x40000000, 0x44700000, 0x3f800000, 0x39888889, 0x3e800000},
	{16777217, 1, 0x4b800000, 0x3f800000, 0x4b000000, 0x3f000000, 0x33000000, 0x3f000000},
	{4294967295u, 0, 0x4f800000, 0x00000000, 0x4f000000, 0x00000000, 0x2f000000, F_INF},
};

static void size_uniforms(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(SIZES) / sizeof(SIZES[0]); i++) {
		const struct size_case *c = &SIZES[i];
		const struct obs_source_async_upload_size s = obs_source_async_upload_size_uniforms(c->cx, c->cy);
		CHECK_BITS(s.width, c->width);
		CHECK_BITS(s.height, c->height);
		CHECK_BITS(s.width_d2, c->width_d2);
		CHECK_BITS(s.height_d2, c->height_d2);
		CHECK_BITS(s.width_x2_i, c->width_x2_i);
		CHECK_BITS(s.height_x2_i, c->height_x2_i);
	}
}

/* The HLG peak level is the nominal peak, clamped to 1000 (BT.2408). At
 * 1000 and below it is kept bit for bit. Characterized, not endorsed: a NaN
 * stays a NaN (the C's > is false), and 0, a negative or -inf pass through
 * (obs_set_video_levels() keeps the level finite and above 0, #2209). */
static void hlg_peak(void **state)
{
	(void)state;
	CHECK_BITS(obs_source_async_upload_hlg_peak(100.0f), 0x42c80000);
	CHECK_BITS(obs_source_async_upload_hlg_peak(999.0f), 0x4479c000);
	CHECK_BITS(obs_source_async_upload_hlg_peak(1000.0f), 0x447a0000);
	CHECK_BITS(obs_source_async_upload_hlg_peak(bits_float(0x447a0001)), 0x447a0000);
	CHECK_BITS(obs_source_async_upload_hlg_peak(1001.0f), 0x447a0000);
	CHECK_BITS(obs_source_async_upload_hlg_peak(10000.0f), 0x447a0000);
	CHECK_BITS(obs_source_async_upload_hlg_peak(bits_float(F_INF)), 0x447a0000);
	CHECK_BITS(obs_source_async_upload_hlg_peak(bits_float(F_NAN)), F_NAN);
	CHECK_BITS(obs_source_async_upload_hlg_peak(bits_float(0xffc00001)), 0xffc00001);
	CHECK_BITS(obs_source_async_upload_hlg_peak(0.0f), 0x00000000);
	CHECK_BITS(obs_source_async_upload_hlg_peak(-0.0f), 0x80000000);
	CHECK_BITS(obs_source_async_upload_hlg_peak(bits_float(0x00000001)), 0x00000001);
	CHECK_BITS(obs_source_async_upload_hlg_peak(-5.0f), 0xc0a00000);
	CHECK_BITS(obs_source_async_upload_hlg_peak(bits_float(F_NEG_INF)), F_NEG_INF);
}

/* An HLG frame's maximum is the HLG peak level, any other transfer's 10000
 * nits, including VIDEO_TRC_DEFAULT and values outside the enum. */
static void maximum_nits(void **state)
{
	(void)state;
	CHECK_BITS(obs_source_async_upload_maximum_nits(VIDEO_TRC_HLG, 1000.0f), 0x447a0000);
	CHECK_BITS(obs_source_async_upload_maximum_nits(VIDEO_TRC_HLG, 100.0f), 0x42c80000);
	CHECK_BITS(obs_source_async_upload_maximum_nits(VIDEO_TRC_HLG, bits_float(F_NAN)), F_NAN);
	const uint8_t others[] = {VIDEO_TRC_DEFAULT, VIDEO_TRC_SRGB, VIDEO_TRC_PQ, 4, 255};
	for (size_t i = 0; i < sizeof(others) / sizeof(others[0]); i++) {
		CHECK_BITS(obs_source_async_upload_maximum_nits(others[i], 1000.0f), 0x461c4000);
		CHECK_BITS(obs_source_async_upload_maximum_nits(others[i], bits_float(F_NAN)), 0x461c4000);
	}
}

struct level_case {
	uint8_t trc;
	float peak, white;
	uint32_t hlg_peak, maximum_nits, over_white;
};

/* The chain obs-source.c runs: the HLG peak from the nominal peak, the
 * maximum nits for the transfer, over the SDR white level. */
static const struct level_case LEVELS[] = {
	{VIDEO_TRC_SRGB, 100, 80, 0x42c80000, 0x461c4000, 0x42fa0000},
	{VIDEO_TRC_SRGB, 100, 300, 0x42c80000, 0x461c4000, 0x42055555},
	{VIDEO_TRC_SRGB, 100, 480, 0x42c80000, 0x461c4000, 0x41a6aaab},
	{VIDEO_TRC_SRGB, 1000, 80, 0x447a0000, 0x461c4000, 0x42fa0000},
	{VIDEO_TRC_SRGB, 1000, 300, 0x447a0000, 0x461c4000, 0x42055555},
	{VIDEO_TRC_SRGB, 1000, 480, 0x447a0000, 0x461c4000, 0x41a6aaab},
	{VIDEO_TRC_SRGB, 1001, 80, 0x447a0000, 0x461c4000, 0x42fa0000},
	{VIDEO_TRC_SRGB, 1001, 300, 0x447a0000, 0x461c4000, 0x42055555},
	{VIDEO_TRC_SRGB, 1001, 480, 0x447a0000, 0x461c4000, 0x41a6aaab},
	{VIDEO_TRC_SRGB, 10000, 80, 0x447a0000, 0x461c4000, 0x42fa0000},
	{VIDEO_TRC_SRGB, 10000, 300, 0x447a0000, 0x461c4000, 0x42055555},
	{VIDEO_TRC_SRGB, 10000, 480, 0x447a0000, 0x461c4000, 0x41a6aaab},
	{VIDEO_TRC_PQ, 100, 80, 0x42c80000, 0x461c4000, 0x42fa0000},
	{VIDEO_TRC_PQ, 100, 300, 0x42c80000, 0x461c4000, 0x42055555},
	{VIDEO_TRC_PQ, 100, 480, 0x42c80000, 0x461c4000, 0x41a6aaab},
	{VIDEO_TRC_PQ, 1000, 80, 0x447a0000, 0x461c4000, 0x42fa0000},
	{VIDEO_TRC_PQ, 1000, 300, 0x447a0000, 0x461c4000, 0x42055555},
	{VIDEO_TRC_PQ, 1000, 480, 0x447a0000, 0x461c4000, 0x41a6aaab},
	{VIDEO_TRC_PQ, 1001, 80, 0x447a0000, 0x461c4000, 0x42fa0000},
	{VIDEO_TRC_PQ, 1001, 300, 0x447a0000, 0x461c4000, 0x42055555},
	{VIDEO_TRC_PQ, 1001, 480, 0x447a0000, 0x461c4000, 0x41a6aaab},
	{VIDEO_TRC_PQ, 10000, 80, 0x447a0000, 0x461c4000, 0x42fa0000},
	{VIDEO_TRC_PQ, 10000, 300, 0x447a0000, 0x461c4000, 0x42055555},
	{VIDEO_TRC_PQ, 10000, 480, 0x447a0000, 0x461c4000, 0x41a6aaab},
	{VIDEO_TRC_HLG, 100, 80, 0x42c80000, 0x42c80000, 0x3fa00000},
	{VIDEO_TRC_HLG, 100, 300, 0x42c80000, 0x42c80000, 0x3eaaaaab},
	{VIDEO_TRC_HLG, 100, 480, 0x42c80000, 0x42c80000, 0x3e555555},
	{VIDEO_TRC_HLG, 1000, 80, 0x447a0000, 0x447a0000, 0x41480000},
	{VIDEO_TRC_HLG, 1000, 300, 0x447a0000, 0x447a0000, 0x40555555},
	{VIDEO_TRC_HLG, 1000, 480, 0x447a0000, 0x447a0000, 0x40055555},
	{VIDEO_TRC_HLG, 1001, 80, 0x447a0000, 0x447a0000, 0x41480000},
	{VIDEO_TRC_HLG, 1001, 300, 0x447a0000, 0x447a0000, 0x40555555},
	{VIDEO_TRC_HLG, 1001, 480, 0x447a0000, 0x447a0000, 0x40055555},
	{VIDEO_TRC_HLG, 10000, 80, 0x447a0000, 0x447a0000, 0x41480000},
	{VIDEO_TRC_HLG, 10000, 300, 0x447a0000, 0x447a0000, 0x40555555},
	{VIDEO_TRC_HLG, 10000, 480, 0x447a0000, 0x447a0000, 0x40055555},
	{4, 100, 80, 0x42c80000, 0x461c4000, 0x42fa0000},
	{4, 100, 300, 0x42c80000, 0x461c4000, 0x42055555},
	{4, 100, 480, 0x42c80000, 0x461c4000, 0x41a6aaab},
	{4, 1000, 80, 0x447a0000, 0x461c4000, 0x42fa0000},
	{4, 1000, 300, 0x447a0000, 0x461c4000, 0x42055555},
	{4, 1000, 480, 0x447a0000, 0x461c4000, 0x41a6aaab},
	{4, 1001, 80, 0x447a0000, 0x461c4000, 0x42fa0000},
	{4, 1001, 300, 0x447a0000, 0x461c4000, 0x42055555},
	{4, 1001, 480, 0x447a0000, 0x461c4000, 0x41a6aaab},
	{4, 10000, 80, 0x447a0000, 0x461c4000, 0x42fa0000},
	{4, 10000, 300, 0x447a0000, 0x461c4000, 0x42055555},
	{4, 10000, 480, 0x447a0000, 0x461c4000, 0x41a6aaab},
};

static void level_uniforms(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(LEVELS) / sizeof(LEVELS[0]); i++) {
		const struct level_case *c = &LEVELS[i];
		const float peak = obs_source_async_upload_hlg_peak(c->peak);
		const float nits = obs_source_async_upload_maximum_nits(c->trc, peak);
		const float over = obs_source_async_upload_over_sdr_white(nits, c->white);
		CHECK_BITS(peak, c->hlg_peak);
		CHECK_BITS(nits, c->maximum_nits);
		CHECK_BITS(over, c->over_white);
	}
}

/* One float division. Characterized, not endorsed: a white level of 0 gives
 * +inf, and 0 over 0 or a NaN gives a NaN (obs_set_video_levels() keeps the
 * level finite and above 0 since #2209). */
static void over_sdr_white_edges(void **state)
{
	(void)state;
	CHECK_BITS(obs_source_async_upload_over_sdr_white(10000.0f, 0.0f), F_INF);
	CHECK_BITS(obs_source_async_upload_over_sdr_white(10000.0f, -0.0f), F_NEG_INF);
	CHECK_BITS(obs_source_async_upload_over_sdr_white(10000.0f, bits_float(F_INF)), 0x00000000);
	CHECK_BOOL(isnan(obs_source_async_upload_over_sdr_white(0.0f, 0.0f)), true);
	CHECK_BITS(obs_source_async_upload_over_sdr_white(bits_float(F_NAN), 300.0f), F_NAN);
	CHECK_BITS(obs_source_async_upload_over_sdr_white(10000.0f, bits_float(F_NAN)), F_NAN);
	/* the smallest subnormal white level overflows */
	CHECK_BITS(obs_source_async_upload_over_sdr_white(10000.0f, bits_float(0x00000001)), F_INF);
	/* rounded once, as the C's float division: 1000 / 3 */
	CHECK_BITS(obs_source_async_upload_over_sdr_white(1000.0f, 3.0f), 0x43a6aaab);
}

/* hdr_lw is the frame's max_luminance, exactly (every uint16_t fits). */
static void hdr_lw(void **state)
{
	(void)state;
	CHECK_BITS(obs_source_async_upload_hdr_lw(0), 0x00000000);
	CHECK_BITS(obs_source_async_upload_hdr_lw(1), 0x3f800000);
	CHECK_BITS(obs_source_async_upload_hdr_lw(1000), 0x447a0000);
	CHECK_BITS(obs_source_async_upload_hdr_lw(10000), 0x461c4000);
	CHECK_BITS(obs_source_async_upload_hdr_lw(65535), 0x477fff00);
	for (uint32_t v = 0; v <= UINT16_MAX; v++)
		CHECK_BITS(obs_source_async_upload_hdr_lw((uint16_t)v), float_bits((float)v));
}

/* Only a limited range frame sets color_range_min and color_range_max. */
static void sets_range(void **state)
{
	(void)state;
	CHECK_BOOL(obs_source_async_upload_sets_range(false), true);
	CHECK_BOOL(obs_source_async_upload_sets_range(true), false);
}

/* --- update_async_textures() ------------------------------------------- */

/* async_linear_alpha is bit 0 of the frame's flags; the other bits don't
 * count. */
static void linear_alpha(void **state)
{
	(void)state;
	for (unsigned flags = 0; flags <= UINT8_MAX; flags++)
		CHECK_BOOL(obs_source_async_upload_linear_alpha((uint8_t)flags), (flags & 1) != 0);
}

/* The texrender draws only with a GPU conversion and a texrender. */
static void uses_texrender(void **state)
{
	(void)state;
	CHECK_BOOL(obs_source_async_upload_uses_texrender(false, false), false);
	CHECK_BOOL(obs_source_async_upload_uses_texrender(false, true), false);
	CHECK_BOOL(obs_source_async_upload_uses_texrender(true, false), false);
	CHECK_BOOL(obs_source_async_upload_uses_texrender(true, true), true);
}

/* --- check_to_swap_bgrx_bgra() ----------------------------------------- */

/* Only a GS_BGRX texture with a VIDEO_FORMAT_BGRA frame (to GS_BGRA) and a
 * GS_BGRA texture with a VIDEO_FORMAT_BGRX frame (to GS_BGRX) swap; every
 * other pair, the _UNORM formats and values outside the enums included,
 * keeps the texture (GS_UNKNOWN). */
static void bgr_swap_every_pair(void **state)
{
	(void)state;
	int swaps = 0;
	for (int tex = -1; tex <= GS_RG16 + 1; tex++) {
		for (int frame = -1; frame <= VIDEO_FORMAT_R10L + 1; frame++) {
			int expected = GS_UNKNOWN;
			if (tex == GS_BGRX && frame == VIDEO_FORMAT_BGRA)
				expected = GS_BGRA;
			else if (tex == GS_BGRA && frame == VIDEO_FORMAT_BGRX)
				expected = GS_BGRX;
			CHECK_INT(obs_source_async_upload_bgr_swap(tex, frame), expected);
			swaps += expected != GS_UNKNOWN;
		}
	}
	CHECK_INT(swaps, 2);
	CHECK_INT(obs_source_async_upload_bgr_swap(GS_BGRX, VIDEO_FORMAT_BGRX), GS_UNKNOWN);
	CHECK_INT(obs_source_async_upload_bgr_swap(GS_BGRA, VIDEO_FORMAT_BGRA), GS_UNKNOWN);
	CHECK_INT(obs_source_async_upload_bgr_swap(GS_BGRX_UNORM, VIDEO_FORMAT_BGRA), GS_UNKNOWN);
	CHECK_INT(obs_source_async_upload_bgr_swap(GS_BGRA_UNORM, VIDEO_FORMAT_BGRX), GS_UNKNOWN);
	CHECK_INT(obs_source_async_upload_bgr_swap(GS_RGBA, VIDEO_FORMAT_RGBA), GS_UNKNOWN);
	CHECK_INT(obs_source_async_upload_bgr_swap(INT_MIN, INT_MAX), GS_UNKNOWN);
}

/* --- obs_source_update_async_video() ----------------------------------- */

/* The timing re-anchors on every frame unless the source is decoupled and
 * unbuffered; async_unbuffered is read only for a decoupled source. */
static void reanchor(void **state)
{
	(void)state;
	CHECK_BOOL(obs_source_async_upload_reanchor_reads_unbuffered(false), false);
	CHECK_BOOL(obs_source_async_upload_reanchor_reads_unbuffered(true), true);
	CHECK_BOOL(obs_source_async_upload_reanchor(false, false), true);
	CHECK_BOOL(obs_source_async_upload_reanchor(false, true), true);
	CHECK_BOOL(obs_source_async_upload_reanchor(true, false), true);
	CHECK_BOOL(obs_source_async_upload_reanchor(true, true), false);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(enum_values_and_layouts),
		cmocka_unit_test(texture_changed_each_field),
		cmocka_unit_test(texture_changed_first_frame),
		cmocka_unit_test(texture_changed_edges),
		cmocka_unit_test(convert_type_routes),
		cmocka_unit_test(size_uniforms),
		cmocka_unit_test(hlg_peak),
		cmocka_unit_test(maximum_nits),
		cmocka_unit_test(level_uniforms),
		cmocka_unit_test(over_sdr_white_edges),
		cmocka_unit_test(hdr_lw),
		cmocka_unit_test(sets_range),
		cmocka_unit_test(linear_alpha),
		cmocka_unit_test(uses_texrender),
		cmocka_unit_test(bgr_swap_every_pair),
		cmocka_unit_test(reanchor),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
