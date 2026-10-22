#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <setjmp.h>
#include <cmocka.h>

#include "libobs/obs-video-plan.c.h"

/* Characterization of the video output plan, before its Rust port. It moved
 * from obs.c into obs-video-plan.c: the size check of obs_reset_video(), the
 * output size alignment, the scale type names of the reset log, and, for
 * each output format and color space, the GPU conversion (shader techniques
 * and texel sizes), each conversion plane's size and texture format (the
 * conversion textures and the copy surfaces), and the format and color space
 * of the render textures. This test compiles the file into itself, so it
 * checks the C. test_video_output_formats runs the same formats through
 * obs_reset_video() and real frames.
 *
 * The expectations below are written out per format, from the code as it
 * was in obs.c, and every combination of format, color space and size is
 * compared field by field. The range is not an input of the plan (it only
 * changes the color matrix).
 *
 * The output size is checked after the alignment as well: an output width
 * of 2 or 3 passes obs_video_size_valid(), but obs_video_align_output_size()
 * masks it to 0, so obs_video_output_size_valid() refuses it, and
 * obs_reset_video() returns OBS_VIDEO_INVALID_PARAM before it frees the old
 * video (#1713). test_video_output_formats checks that through the real
 * libobs.
 */

#define N(a) (sizeof(a) / sizeof((a)[0]))

/* every enum video_format value, and one past the end */
static const enum video_format all_formats[] = {
	VIDEO_FORMAT_NONE, VIDEO_FORMAT_I420, VIDEO_FORMAT_NV12,
	VIDEO_FORMAT_YVYU, VIDEO_FORMAT_YUY2, VIDEO_FORMAT_UYVY,
	VIDEO_FORMAT_RGBA, VIDEO_FORMAT_BGRA, VIDEO_FORMAT_BGRX,
	VIDEO_FORMAT_Y800, VIDEO_FORMAT_I444, VIDEO_FORMAT_BGR3,
	VIDEO_FORMAT_I422, VIDEO_FORMAT_I40A, VIDEO_FORMAT_I42A,
	VIDEO_FORMAT_YUVA, VIDEO_FORMAT_AYUV, VIDEO_FORMAT_I010,
	VIDEO_FORMAT_P010, VIDEO_FORMAT_I210, VIDEO_FORMAT_I412,
	VIDEO_FORMAT_YA2L, VIDEO_FORMAT_P216, VIDEO_FORMAT_P416,
	VIDEO_FORMAT_V210, VIDEO_FORMAT_R10L, (enum video_format)(VIDEO_FORMAT_R10L + 1),
};

/* every enum video_colorspace value, and one past the end */
static const enum video_colorspace all_colorspaces[] = {
	VIDEO_CS_DEFAULT,
	VIDEO_CS_601,
	VIDEO_CS_709,
	VIDEO_CS_SRGB,
	VIDEO_CS_2100_PQ,
	VIDEO_CS_2100_HLG,
	(enum video_colorspace)(VIDEO_CS_2100_HLG + 1),
};

/* odd and even, the smallest, the 1366 that aligns to 1364, the largest */
static const uint32_t sizes[] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 767, 1080, 1081, 1364, 1366, 1920, 32767, 32768};

/* ------------------------------------------------------------------------- */
/* expectations, per format */

enum texel {
	TEXEL_NONE,  /* conversion_width_i and _height_i stay 0 */
	TEXEL_WIDTH, /* only conversion_width_i */
	TEXEL_BOTH,
};

struct plane_rule {
	/* the plane is width / wdiv by height / hdiv; 0 means no plane */
	uint32_t wdiv;
	uint32_t hdiv;
	enum gs_color_format format;
};

struct format_rule {
	enum video_format format;
	/* techniques for SDR, PQ and HLG; one set for every color space when
	 * the PQ and HLG sets are NULL */
	const char *techs[3][3];
	enum texel texel;
	struct plane_rule planes[3];
};

#define SDR 0
#define PQ 1
#define HLG 2

static const struct format_rule rules[] = {
	{VIDEO_FORMAT_I420,
	 {{"Planar_Y", "Planar_U_Left", "Planar_V_Left"}},
	 TEXEL_WIDTH,
	 {{1, 1, GS_R8}, {2, 2, GS_R8}, {2, 2, GS_R8}}},
	{VIDEO_FORMAT_NV12, {{"NV12_Y", "NV12_UV", NULL}}, TEXEL_WIDTH, {{1, 1, GS_R8}, {2, 2, GS_R8G8}}},
	{VIDEO_FORMAT_I444,
	 {{"Planar_Y", "Planar_U", "Planar_V"}},
	 TEXEL_NONE,
	 {{1, 1, GS_R8}, {1, 1, GS_R8}, {1, 1, GS_R8}}},
	{VIDEO_FORMAT_I010,
	 {{"I010_SRGB_Y", "I010_SRGB_U", "I010_SRGB_V"},
	  {"I010_PQ_Y", "I010_PQ_U", "I010_PQ_V"},
	  {"I010_HLG_Y", "I010_HLG_U", "I010_HLG_V"}},
	 TEXEL_BOTH,
	 {{1, 1, GS_R16}, {2, 2, GS_R16}, {2, 2, GS_R16}}},
	{VIDEO_FORMAT_P010,
	 {{"P010_SRGB_Y", "P010_SRGB_UV", NULL}, {"P010_PQ_Y", "P010_PQ_UV", NULL}, {"P010_HLG_Y", "P010_HLG_UV", NULL}},
	 TEXEL_BOTH,
	 {{1, 1, GS_R16}, {2, 2, GS_RG16}}},
	{VIDEO_FORMAT_P216,
	 {{"P216_SRGB_Y", "P216_SRGB_UV", NULL}, {"P216_PQ_Y", "P216_PQ_UV", NULL}, {"P216_HLG_Y", "P216_HLG_UV", NULL}},
	 TEXEL_BOTH,
	 {{1, 1, GS_R16}, {2, 1, GS_RG16}}},
	{VIDEO_FORMAT_P416,
	 {{"P416_SRGB_Y", "P416_SRGB_UV", NULL}, {"P416_PQ_Y", "P416_PQ_UV", NULL}, {"P416_HLG_Y", "P416_HLG_UV", NULL}},
	 TEXEL_BOTH,
	 {{1, 1, GS_R16}, {1, 1, GS_RG16}}},
};

static const struct format_rule *find_rule(enum video_format format)
{
	for (size_t i = 0; i < N(rules); i++) {
		if (rules[i].format == format)
			return &rules[i];
	}
	return NULL;
}

static bool is_hdr(enum video_colorspace cs)
{
	return cs == VIDEO_CS_2100_PQ || cs == VIDEO_CS_2100_HLG;
}

static bool high_precision(enum video_format format)
{
	switch (format) {
	case VIDEO_FORMAT_I010:
	case VIDEO_FORMAT_P010:
	case VIDEO_FORMAT_I210:
	case VIDEO_FORMAT_I412:
	case VIDEO_FORMAT_YA2L:
	case VIDEO_FORMAT_P216:
	case VIDEO_FORMAT_P416:
		return true;
	default:
		return false;
	}
}

static struct obs_video_output_plan expected_plan(enum video_format format, enum video_colorspace cs, uint32_t width,
						  uint32_t height)
{
	struct obs_video_output_plan plan;
	memset(&plan, 0, sizeof(plan));

	const struct format_rule *rule = find_rule(format);
	if (rule) {
		size_t set = SDR;
		if (rule->techs[PQ][0] && cs == VIDEO_CS_2100_PQ)
			set = PQ;
		else if (rule->techs[HLG][0] && cs == VIDEO_CS_2100_HLG)
			set = HLG;

		plan.conversion_needed = true;
		for (size_t c = 0; c < 3; c++) {
			plan.conversion_techs[c] = rule->techs[set][c];
			if (rule->planes[c].wdiv)
				plan.planes[c] = (struct obs_video_plane){width / rule->planes[c].wdiv,
									  height / rule->planes[c].hdiv,
									  rule->planes[c].format};
		}
		if (rule->texel != TEXEL_NONE)
			plan.conversion_width_i = 1.f / (float)width;
		if (rule->texel == TEXEL_BOTH)
			plan.conversion_height_i = 1.f / (float)height;
	}

	plan.texture_format = high_precision(format) ? GS_RGBA16F : GS_BGRA;
	if (is_hdr(cs))
		plan.render_space = GS_CS_709_EXTENDED;
	else if (format == VIDEO_FORMAT_I010 || format == VIDEO_FORMAT_P010 || format == VIDEO_FORMAT_P216 ||
		 format == VIDEO_FORMAT_P416)
		plan.render_space = GS_CS_SRGB_16F;
	else
		plan.render_space = GS_CS_SRGB;
	return plan;
}

static bool same_float(float a, float b)
{
	return memcmp(&a, &b, sizeof(a)) == 0;
}

static bool same_tech(const char *a, const char *b)
{
	if (!a || !b)
		return a == b;
	return strcmp(a, b) == 0;
}

static void check_plan(enum video_format format, enum video_colorspace cs, uint32_t width, uint32_t height)
{
	const struct obs_video_output_plan want = expected_plan(format, cs, width, height);
	const struct obs_video_output_plan got = obs_video_output_plan(format, cs, width, height);
	char what[96];
	snprintf(what, sizeof(what), "format %d, colorspace %d, %ux%u", (int)format, (int)cs, width, height);

	bool ok = got.conversion_needed == want.conversion_needed &&
		  same_float(got.conversion_width_i, want.conversion_width_i) &&
		  same_float(got.conversion_height_i, want.conversion_height_i) &&
		  got.texture_format == want.texture_format && got.render_space == want.render_space;
	for (size_t c = 0; c < 3; c++) {
		ok = ok && same_tech(got.conversion_techs[c], want.conversion_techs[c]);
		ok = ok && got.planes[c].width == want.planes[c].width &&
		     got.planes[c].height == want.planes[c].height && got.planes[c].format == want.planes[c].format;
	}
	if (!ok)
		fail_msg("plan differs for %s", what);
}

/* ------------------------------------------------------------------------- */
/* the plan, for every combination */

static void every_format_colorspace_and_size(void **state)
{
	(void)state;
	size_t checked = 0;
	for (size_t f = 0; f < N(all_formats); f++) {
		for (size_t c = 0; c < N(all_colorspaces); c++) {
			for (size_t w = 0; w < N(sizes); w++) {
				for (size_t h = 0; h < N(sizes); h++) {
					check_plan(all_formats[f], all_colorspaces[c], sizes[w], sizes[h]);
					checked++;
				}
			}
		}
	}
	assert_int_equal(checked, N(all_formats) * N(all_colorspaces) * N(sizes) * N(sizes));
}

/* The same plan, written out for the common sizes, so a reader sees the
 * numbers. */
static void common_sizes_written_out(void **state)
{
	(void)state;

	struct obs_video_output_plan p = obs_video_output_plan(VIDEO_FORMAT_NV12, VIDEO_CS_709, 1920, 1080);
	assert_true(p.conversion_needed);
	assert_string_equal(p.conversion_techs[0], "NV12_Y");
	assert_string_equal(p.conversion_techs[1], "NV12_UV");
	assert_null(p.conversion_techs[2]);
	assert_true(same_float(p.conversion_width_i, 1.f / 1920.f));
	assert_true(same_float(p.conversion_height_i, 0.f));
	assert_int_equal(p.planes[0].width, 1920);
	assert_int_equal(p.planes[0].height, 1080);
	assert_int_equal(p.planes[0].format, GS_R8);
	assert_int_equal(p.planes[1].width, 960);
	assert_int_equal(p.planes[1].height, 540);
	assert_int_equal(p.planes[1].format, GS_R8G8);
	assert_int_equal(p.planes[2].width, 0);
	assert_int_equal(p.planes[2].height, 0);
	assert_int_equal(p.planes[2].format, GS_UNKNOWN);
	assert_int_equal(p.texture_format, GS_BGRA);
	assert_int_equal(p.render_space, GS_CS_SRGB);

	p = obs_video_output_plan(VIDEO_FORMAT_I420, VIDEO_CS_601, 1280, 720);
	assert_string_equal(p.conversion_techs[0], "Planar_Y");
	assert_string_equal(p.conversion_techs[1], "Planar_U_Left");
	assert_string_equal(p.conversion_techs[2], "Planar_V_Left");
	assert_true(same_float(p.conversion_width_i, 1.f / 1280.f));
	assert_int_equal(p.planes[1].width, 640);
	assert_int_equal(p.planes[1].height, 360);
	assert_int_equal(p.planes[2].width, 640);
	assert_int_equal(p.planes[2].height, 360);
	assert_int_equal(p.planes[2].format, GS_R8);

	p = obs_video_output_plan(VIDEO_FORMAT_I444, VIDEO_CS_SRGB, 1280, 720);
	assert_string_equal(p.conversion_techs[1], "Planar_U");
	assert_string_equal(p.conversion_techs[2], "Planar_V");
	assert_true(same_float(p.conversion_width_i, 0.f));
	assert_int_equal(p.planes[2].width, 1280);
	assert_int_equal(p.planes[2].height, 720);

	p = obs_video_output_plan(VIDEO_FORMAT_P010, VIDEO_CS_2100_PQ, 3840, 2160);
	assert_string_equal(p.conversion_techs[0], "P010_PQ_Y");
	assert_string_equal(p.conversion_techs[1], "P010_PQ_UV");
	assert_null(p.conversion_techs[2]);
	assert_true(same_float(p.conversion_width_i, 1.f / 3840.f));
	assert_true(same_float(p.conversion_height_i, 1.f / 2160.f));
	assert_int_equal(p.planes[0].format, GS_R16);
	assert_int_equal(p.planes[1].width, 1920);
	assert_int_equal(p.planes[1].height, 1080);
	assert_int_equal(p.planes[1].format, GS_RG16);
	assert_int_equal(p.texture_format, GS_RGBA16F);
	assert_int_equal(p.render_space, GS_CS_709_EXTENDED);

	p = obs_video_output_plan(VIDEO_FORMAT_I010, VIDEO_CS_2100_HLG, 1920, 1080);
	assert_string_equal(p.conversion_techs[2], "I010_HLG_V");
	assert_int_equal(p.planes[2].width, 960);
	assert_int_equal(p.planes[2].height, 540);
	assert_int_equal(p.planes[2].format, GS_R16);

	p = obs_video_output_plan(VIDEO_FORMAT_P216, VIDEO_CS_709, 1920, 1080);
	assert_string_equal(p.conversion_techs[0], "P216_SRGB_Y");
	assert_int_equal(p.planes[1].width, 960);
	assert_int_equal(p.planes[1].height, 1080);
	assert_int_equal(p.render_space, GS_CS_SRGB_16F);

	p = obs_video_output_plan(VIDEO_FORMAT_P416, VIDEO_CS_DEFAULT, 1920, 1080);
	assert_string_equal(p.conversion_techs[1], "P416_SRGB_UV");
	assert_int_equal(p.planes[1].width, 1920);
	assert_int_equal(p.planes[1].height, 1080);
}

/* Odd sizes round the chroma planes down. obs_reset_video() aligns the main
 * output to a width that is a multiple of 4 and an even height, but a
 * canvas's mix (obs_canvas_create(), obs_canvas_reset_video()) is not
 * aligned, so its plan can see any size. */
static void odd_sizes_round_chroma_down(void **state)
{
	(void)state;
	struct obs_video_output_plan p = obs_video_output_plan(VIDEO_FORMAT_I420, VIDEO_CS_709, 7, 5);
	assert_int_equal(p.planes[0].width, 7);
	assert_int_equal(p.planes[0].height, 5);
	assert_int_equal(p.planes[1].width, 3);
	assert_int_equal(p.planes[1].height, 2);
	assert_true(same_float(p.conversion_width_i, 1.f / 7.f));

	p = obs_video_output_plan(VIDEO_FORMAT_P216, VIDEO_CS_709, 7, 5);
	assert_int_equal(p.planes[1].width, 3);
	assert_int_equal(p.planes[1].height, 5);
	assert_true(same_float(p.conversion_height_i, 1.f / 5.f));

	/* a 1x1 output gets empty chroma planes */
	p = obs_video_output_plan(VIDEO_FORMAT_NV12, VIDEO_CS_709, 1, 1);
	assert_int_equal(p.planes[1].width, 0);
	assert_int_equal(p.planes[1].height, 0);
	assert_int_equal(p.planes[1].format, GS_R8G8);
}

/* Formats without a GPU conversion: no techniques and no planes, only the
 * render texture format and space. */
static void formats_without_conversion(void **state)
{
	(void)state;
	const enum video_format none[] = {
		VIDEO_FORMAT_NONE, VIDEO_FORMAT_YVYU, VIDEO_FORMAT_YUY2, VIDEO_FORMAT_UYVY, VIDEO_FORMAT_RGBA,
		VIDEO_FORMAT_BGRA, VIDEO_FORMAT_BGRX, VIDEO_FORMAT_Y800, VIDEO_FORMAT_BGR3, VIDEO_FORMAT_I422,
		VIDEO_FORMAT_I40A, VIDEO_FORMAT_I42A, VIDEO_FORMAT_YUVA, VIDEO_FORMAT_AYUV, VIDEO_FORMAT_I210,
		VIDEO_FORMAT_I412, VIDEO_FORMAT_YA2L, VIDEO_FORMAT_V210, VIDEO_FORMAT_R10L,
	};
	for (size_t i = 0; i < N(none); i++) {
		struct obs_video_output_plan p = obs_video_output_plan(none[i], VIDEO_CS_709, 64, 64);
		assert_false(p.conversion_needed);
		for (size_t c = 0; c < 3; c++) {
			assert_null(p.conversion_techs[c]);
			assert_int_equal(p.planes[c].format, GS_UNKNOWN);
			assert_int_equal(p.planes[c].width, 0);
		}
		assert_true(same_float(p.conversion_width_i, 0.f));
		assert_true(same_float(p.conversion_height_i, 0.f));
	}

	/* I210, I412 and YA2L render in 16-bit float, but in SDR space */
	struct obs_video_output_plan p = obs_video_output_plan(VIDEO_FORMAT_I210, VIDEO_CS_709, 64, 64);
	assert_int_equal(p.texture_format, GS_RGBA16F);
	assert_int_equal(p.render_space, GS_CS_SRGB);
	p = obs_video_output_plan(VIDEO_FORMAT_YA2L, VIDEO_CS_2100_PQ, 64, 64);
	assert_int_equal(p.texture_format, GS_RGBA16F);
	assert_int_equal(p.render_space, GS_CS_709_EXTENDED);
	/* an 8-bit format with an HDR color space still renders in HDR space */
	p = obs_video_output_plan(VIDEO_FORMAT_BGRA, VIDEO_CS_2100_HLG, 64, 64);
	assert_int_equal(p.texture_format, GS_BGRA);
	assert_int_equal(p.render_space, GS_CS_709_EXTENDED);
	/* 8-bit formats ignore PQ and HLG for their techniques */
	p = obs_video_output_plan(VIDEO_FORMAT_NV12, VIDEO_CS_2100_PQ, 64, 64);
	assert_string_equal(p.conversion_techs[0], "NV12_Y");
	assert_int_equal(p.render_space, GS_CS_709_EXTENDED);
}

/* Every technique the plan names is a technique of format_conversion.effect,
 * which obs-video.c loads them from by name. */
static void techniques_exist_in_the_effect(void **state)
{
	(void)state;
#ifndef FORMAT_CONVERSION_EFFECT
	/* The cc-build (c-tests-linux.json) runs from a private working
	 * directory and does not pass the effect path; the CMake build does,
	 * and the real-libobs test_video_output_formats loads the effect too. */
	skip();
#else
	FILE *f = fopen(FORMAT_CONVERSION_EFFECT, "rb");
	assert_non_null(f);
	fseek(f, 0, SEEK_END);
	long len = ftell(f);
	fseek(f, 0, SEEK_SET);
	char *text = malloc((size_t)len + 1);
	assert_non_null(text);
	assert_int_equal(fread(text, 1, (size_t)len, f), (size_t)len);
	text[len] = 0;
	fclose(f);

	size_t named = 0;
	for (size_t i = 0; i < N(all_formats); i++) {
		for (size_t c = 0; c < N(all_colorspaces); c++) {
			struct obs_video_output_plan p =
				obs_video_output_plan(all_formats[i], all_colorspaces[c], 64, 64);
			for (size_t t = 0; t < 3; t++) {
				if (!p.conversion_techs[t])
					continue;
				char needle[64];
				snprintf(needle, sizeof(needle), "\ntechnique %s\n", p.conversion_techs[t]);
				if (!strstr(text, needle))
					fail_msg("technique %s is not in format_conversion.effect",
						 p.conversion_techs[t]);
				named++;
			}
		}
	}
	assert_true(named > 0);
	free(text);
#endif
}

/* ------------------------------------------------------------------------- */
/* the size check and the alignment of obs_reset_video() */

static void size_check_edges(void **state)
{
	(void)state;
	const struct {
		uint32_t size;
		bool valid;
	} cases[] = {
		{0, false},    {1, false},    {2, true},      {3, true},      {4, true},           {1366, true},
		{32767, true}, {32768, true}, {32769, false}, {65536, false}, {UINT32_MAX, false},
	};
	for (size_t w = 0; w < N(cases); w++) {
		for (size_t h = 0; h < N(cases); h++) {
			bool want = cases[w].valid && cases[h].valid;
			if (obs_video_size_valid(cases[w].size, cases[h].size) != want)
				fail_msg("size_valid(%u, %u) is not %d", cases[w].size, cases[h].size, want);
		}
	}
}

static void alignment_masks(void **state)
{
	(void)state;
	const struct {
		uint32_t in;
		uint32_t width;
		uint32_t height;
	} cases[] = {
		{0, 0, 0},
		{1, 0, 0},
		{4, 4, 4},
		{5, 4, 4},
		{6, 4, 6},
		{7, 4, 6},
		{8, 8, 8},
		{9, 8, 8},
		{767, 764, 766},
		{1080, 1080, 1080},
		{1366, 1364, 1366},
		{1367, 1364, 1366},
		{32767, 32764, 32766},
		{32768, 32768, 32768},
		{UINT32_MAX, 0xFFFFFFFC, 0xFFFFFFFE},
	};
	for (size_t i = 0; i < N(cases); i++) {
		uint32_t width = cases[i].in;
		uint32_t height = cases[i].in;
		obs_video_align_output_size(&width, &height);
		assert_int_equal(width, cases[i].width);
		assert_int_equal(height, cases[i].height);
	}

	/* the two are independent */
	uint32_t width = 1366, height = 767;
	obs_video_align_output_size(&width, &height);
	assert_int_equal(width, 1364);
	assert_int_equal(height, 766);
}

/* A width of 2 or 3 passes the size check, and the alignment makes it 0,
 * so the output size check refuses it. The height minimum, 2, survives the
 * alignment, and 3 becomes 2, so those heights stay valid. */
static void width_2_and_3_refused_after_alignment(void **state)
{
	(void)state;
	for (uint32_t w = 2; w <= 3; w++) {
		assert_true(obs_video_size_valid(w, 2));
		uint32_t width = w, height = 2;
		obs_video_align_output_size(&width, &height);
		assert_int_equal(width, 0);
		assert_int_equal(height, 2);
		for (uint32_t h = 2; h <= 3; h++)
			assert_false(obs_video_output_size_valid(w, h));
	}
	uint32_t width = 4, height = 3;
	obs_video_align_output_size(&width, &height);
	assert_int_equal(width, 4);
	assert_int_equal(height, 2);
	assert_true(obs_video_output_size_valid(4, 2));
	assert_true(obs_video_output_size_valid(4, 3));
	assert_true(obs_video_output_size_valid(5, 3));
}

/* Every other size is accepted or refused as by obs_video_size_valid(): only
 * the widths 2 and 3 change. The alignment only lowers a size, so a size
 * above the maximum stays refused. */
static void output_size_check_edges(void **state)
{
	(void)state;
	const uint32_t sizes[] = {0,    1,     2,     3,     4,     5,     6,     7,         8,
				  1366, 32767, 32768, 32769, 32771, 32772, 65536, UINT32_MAX};
	for (size_t w = 0; w < N(sizes); w++) {
		for (size_t h = 0; h < N(sizes); h++) {
			bool want = obs_video_size_valid(sizes[w], sizes[h]) && sizes[w] != 2 && sizes[w] != 3;
			if (obs_video_output_size_valid(sizes[w], sizes[h]) != want)
				fail_msg("output_size_valid(%u, %u) is not %d", sizes[w], sizes[h], want);
		}
	}
}

/* ------------------------------------------------------------------------- */

static void scale_type_names(void **state)
{
	(void)state;
	assert_string_equal(obs_video_scale_type_name(OBS_SCALE_DISABLE), "Disabled");
	assert_string_equal(obs_video_scale_type_name(OBS_SCALE_POINT), "Point");
	assert_string_equal(obs_video_scale_type_name(OBS_SCALE_BICUBIC), "Bicubic");
	assert_string_equal(obs_video_scale_type_name(OBS_SCALE_BILINEAR), "Bilinear");
	assert_string_equal(obs_video_scale_type_name(OBS_SCALE_LANCZOS), "Lanczos");
	assert_string_equal(obs_video_scale_type_name(OBS_SCALE_AREA), "Area");
	/* any other value logs an empty name */
	assert_string_equal(obs_video_scale_type_name((enum obs_scale_type)(OBS_SCALE_AREA + 1)), "");
	assert_string_equal(obs_video_scale_type_name((enum obs_scale_type) - 1), "");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(every_format_colorspace_and_size),
		cmocka_unit_test(common_sizes_written_out),
		cmocka_unit_test(odd_sizes_round_chroma_down),
		cmocka_unit_test(formats_without_conversion),
		cmocka_unit_test(techniques_exist_in_the_effect),
		cmocka_unit_test(size_check_edges),
		cmocka_unit_test(alignment_masks),
		cmocka_unit_test(width_2_and_3_refused_after_alignment),
		cmocka_unit_test(output_size_check_edges),
		cmocka_unit_test(scale_type_names),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
