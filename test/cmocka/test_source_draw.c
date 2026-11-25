/* Characterization of obs_source_draw_set_color_matrix() and
 * obs_source_draw() (libobs/obs-source-draw.c.h, #5742) through the real
 * libobs and an OpenGL device.
 *
 * obs_source_draw_set_color_matrix(matrix, min, max):
 *   - with no current effect it logs one LOG_WARNING ("no active effect!")
 *     and sets nothing, also for a NULL matrix (the effect is checked
 *     first);
 *   - a NULL matrix logs obs_ptr_valid()'s LOG_DEBUG line ("Null
 *     'color_matrix' parameter") and sets nothing;
 *   - otherwise it sets the current effect's "color_matrix" to the 64-byte
 *     matrix and "color_range_min" / "color_range_max" to 12 bytes each, a
 *     NULL min or max as (0, 0, 0) / (1, 1, 1), and logs nothing.
 *
 * obs_source_draw(texture, x, y, cx, cy, flip):
 *   - a NULL texture logs obs_ptr_valid()'s LOG_DEBUG line ("Null 'texture'
 *     parameter") and draws nothing, also with no current effect (the
 *     texture is checked first);
 *   - with no current effect it logs one LOG_WARNING ("no active effect!")
 *     and draws nothing;
 *   - otherwise it sets the current effect's "image" to the texture, marked
 *     sRGB exactly when gs_get_linear_srgb() is on, and draws the texture as
 *     a sprite at (x, y), of its own size when cx and cy are 0, else cx by
 *     cy, upside down when flip is set;
 *   - the framebuffer sRGB flag and the matrix are what they were before
 *     the call, for every flag and linear sRGB combination.
 *
 * Left to the Rust swap's Tier 3 call-sequence parity, not observable here:
 * the order of the three param lookups, that the framebuffer sRGB flag is
 * set to gs_get_linear_srgb() during the draw, and that the matrix is pushed
 * and translated only when x or y is not 0 (the matrix after the call is
 * the same either way).
 *
 * The cases make the OpenGL device with gs_create() and enter its context
 * on the test thread, without obs_startup(). The device needs a display on
 * Linux, so the test runs under xvfb-run there. It skips only when no
 * display is set at all; with one set, a failed gs_create() fails it.
 * Colours are 0 or 1 so the sRGB transfer leaves them alone. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <graphics/matrix4.h>
#include <graphics/vec3.h>
#include <graphics/vec4.h>
#include <util/base.h>
#include <util/bmem.h>

#define SIZE 8
#define TOLERANCE 2

static graphics_t *graphics;
static gs_effect_t *effect;
static gs_texture_t *texture;

/* An image param and the three colour params, each used by a technique. */
static const char effect_text[] = "uniform float4x4 ViewProj;\n"
				  "uniform texture2d image;\n"
				  "uniform float4x4 color_matrix;\n"
				  "uniform float3 color_range_min = {0.0, 0.0, 0.0};\n"
				  "uniform float3 color_range_max = {1.0, 1.0, 1.0};\n"
				  "\n"
				  "sampler_state def_sampler {\n"
				  "\tFilter = Point;\n"
				  "\tAddressU = Clamp;\n"
				  "\tAddressV = Clamp;\n"
				  "};\n"
				  "\n"
				  "struct VertInOut {\n"
				  "\tfloat4 pos : POSITION;\n"
				  "\tfloat2 uv : TEXCOORD0;\n"
				  "};\n"
				  "\n"
				  "VertInOut VSDefault(VertInOut vert_in)\n"
				  "{\n"
				  "\tVertInOut vert_out;\n"
				  "\tvert_out.pos = mul(float4(vert_in.pos.xyz, 1.0), ViewProj);\n"
				  "\tvert_out.uv = vert_in.uv;\n"
				  "\treturn vert_out;\n"
				  "}\n"
				  "\n"
				  "float4 PSDraw(VertInOut vert_in) : TARGET\n"
				  "{\n"
				  "\treturn image.Sample(def_sampler, vert_in.uv);\n"
				  "}\n"
				  "\n"
				  "float4 PSMatrix(VertInOut vert_in) : TARGET\n"
				  "{\n"
				  "\tfloat4 rgba = image.Sample(def_sampler, vert_in.uv);\n"
				  "\tfloat3 rgb = clamp(rgba.rgb, color_range_min, color_range_max);\n"
				  "\treturn float4(mul(float4(rgb, 1.0), color_matrix).rgb, rgba.a);\n"
				  "}\n"
				  "\n"
				  "technique Draw\n"
				  "{\n"
				  "\tpass\n"
				  "\t{\n"
				  "\t\tvertex_shader = VSDefault(vert_in);\n"
				  "\t\tpixel_shader  = PSDraw(vert_in);\n"
				  "\t}\n"
				  "}\n"
				  "\n"
				  "technique Matrix\n"
				  "{\n"
				  "\tpass\n"
				  "\t{\n"
				  "\t\tvertex_shader = VSDefault(vert_in);\n"
				  "\t\tpixel_shader  = PSMatrix(vert_in);\n"
				  "\t}\n"
				  "}\n";

/* The 2x2 texture, RGBA bytes, rows top first: red, green / blue, white. */
static const uint8_t texels[2][2][4] = {
	{{255, 0, 0, 255}, {0, 255, 0, 255}},
	{{0, 0, 255, 255}, {255, 255, 255, 255}},
};

/* ------------------------------------------------------------------------- */
/* the log lines of the two functions */

#define MAX_LINES 16

struct line {
	int level;
	char text[256];
};

static struct line lines[MAX_LINES];
static int line_count;

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	char text[256];
	vsnprintf(text, sizeof(text), format, args);
	if (strncmp(text, "obs_source_draw", strlen("obs_source_draw")) != 0)
		return;
	if (line_count < MAX_LINES) {
		lines[line_count].level = level;
		snprintf(lines[line_count].text, sizeof(lines[line_count].text), "%s", text);
	}
	line_count++;
}

static void start_log(void)
{
	line_count = 0;
	base_set_log_handler(capture_log, NULL);
}

static void stop_log(void)
{
	base_set_log_handler(NULL, NULL);
}

/* Exactly one line was logged, this one at this level. */
static void assert_one_line(int level, const char *text)
{
	if (line_count != 1 || lines[0].level != level || strcmp(lines[0].text, text) != 0) {
		fprintf(stderr, "want one line at %d: \"%s\"; got %d:\n", level, text, line_count);
		for (int i = 0; i < line_count && i < MAX_LINES; i++)
			fprintf(stderr, "  %d: \"%s\"\n", lines[i].level, lines[i].text);
		fail();
	}
}

static void assert_no_line(void)
{
	if (line_count != 0) {
		fprintf(stderr, "want no line; got %d, the first at %d: \"%s\"\n", line_count, lines[0].level,
			lines[0].text);
		fail();
	}
}

/* ------------------------------------------------------------------------- */
/* params */

static gs_eparam_t *param(const char *name)
{
	gs_eparam_t *p = gs_effect_get_param_by_name(effect, name);
	assert_non_null(p);
	return p;
}

/* The param's current value is size bytes equal to want. */
static bool param_is(const char *name, const void *want, size_t size)
{
	gs_eparam_t *p = param(name);
	if (gs_effect_get_val_size(p) != size)
		return false;
	void *got = gs_effect_get_val(p);
	bool same = got && memcmp(got, want, size) == 0;
	bfree(got);
	return same;
}

/* The param has no current value (set nothing since the technique began). */
static bool param_unset(const char *name)
{
	return gs_effect_get_val_size(param(name)) == 0;
}

static void fill_matrix(struct matrix4 *m, float base)
{
	float *f = (float *)m;
	for (int i = 0; i < 16; i++)
		f[i] = base + (float)i;
}

/* Begins the Matrix technique, so its effect is the current one, without a
 * pass; ends it with end_technique(). */
static gs_technique_t *begin_technique(void)
{
	gs_technique_t *tech = gs_effect_get_technique(effect, "Matrix");
	assert_non_null(tech);
	assert_true(gs_technique_begin(tech) > 0);
	assert_ptr_equal(gs_get_effect(), effect);
	return tech;
}

static void end_technique(gs_technique_t *tech)
{
	gs_technique_end(tech);
	assert_null(gs_get_effect());
}

/* ------------------------------------------------------------------------- */
/* obs_source_draw_set_color_matrix */

/* No current effect: one warning, nothing set, also for a NULL matrix. */
static void color_matrix_without_effect_warns(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	struct matrix4 m;
	struct vec3 min, max;
	fill_matrix(&m, 1.0f);
	vec3_set(&min, 0.25f, 0.5f, 0.75f);
	vec3_set(&max, 0.5f, 0.75f, 1.0f);

	gs_enter_context(graphics);
	assert_null(gs_get_effect());

	start_log();
	obs_source_draw_set_color_matrix(&m, &min, &max);
	stop_log();
	assert_one_line(LOG_WARNING, "obs_source_draw_set_color_matrix: no active effect!");

	start_log();
	obs_source_draw_set_color_matrix(NULL, NULL, NULL);
	stop_log();
	assert_one_line(LOG_WARNING, "obs_source_draw_set_color_matrix: no active effect!");

	gs_technique_t *tech = begin_technique();
	bool unset = param_unset("color_matrix") && param_unset("color_range_min") && param_unset("color_range_max");
	end_technique(tech);
	gs_leave_context();

	assert_true(unset);
}

/* A NULL matrix: obs_ptr_valid()'s debug line, nothing set, also with a
 * min and max given. */
static void color_matrix_null_matrix_sets_nothing(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	struct matrix4 before;
	struct vec3 min_before, max_before, min, max;
	fill_matrix(&before, 100.0f);
	vec3_set(&min_before, 0.125f, 0.25f, 0.375f);
	vec3_set(&max_before, 0.625f, 0.75f, 0.875f);
	vec3_set(&min, 0.25f, 0.5f, 0.75f);
	vec3_set(&max, 0.5f, 0.75f, 1.0f);

	gs_enter_context(graphics);
	gs_technique_t *tech = begin_technique();
	gs_effect_set_matrix4(param("color_matrix"), &before);
	gs_effect_set_val(param("color_range_min"), &min_before, sizeof(float) * 3);
	gs_effect_set_val(param("color_range_max"), &max_before, sizeof(float) * 3);

	start_log();
	obs_source_draw_set_color_matrix(NULL, &min, &max);
	stop_log();
	bool kept = param_is("color_matrix", &before, sizeof(before)) &&
		    param_is("color_range_min", &min_before, sizeof(float) * 3) &&
		    param_is("color_range_max", &max_before, sizeof(float) * 3);
	int count = line_count;
	struct line first = lines[0];

	start_log();
	obs_source_draw_set_color_matrix(NULL, NULL, NULL);
	stop_log();
	kept = kept && param_is("color_matrix", &before, sizeof(before)) &&
	       param_is("color_range_min", &min_before, sizeof(float) * 3) &&
	       param_is("color_range_max", &max_before, sizeof(float) * 3);
	end_technique(tech);
	gs_leave_context();

	assert_true(kept);
	assert_int_equal(count, 1);
	assert_int_equal(first.level, LOG_DEBUG);
	assert_string_equal(first.text, "obs_source_draw_set_color_matrix: Null 'color_matrix' parameter");
	assert_one_line(LOG_DEBUG, "obs_source_draw_set_color_matrix: Null 'color_matrix' parameter");
}

/* Sets the matrix and both ranges as given, 64 and 12 bytes, and logs
 * nothing. */
static void color_matrix_sets_matrix_and_ranges(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	struct matrix4 m;
	struct vec3 min, max;
	fill_matrix(&m, -3.5f);
	vec3_set(&min, 0.0625f, 0.125f, 0.25f);
	vec3_set(&max, 0.5f, 0.75f, 0.9375f);

	gs_enter_context(graphics);
	gs_technique_t *tech = begin_technique();
	start_log();
	obs_source_draw_set_color_matrix(&m, &min, &max);
	stop_log();
	bool matrix = param_is("color_matrix", &m, sizeof(m));
	bool range_min = param_is("color_range_min", &min, sizeof(float) * 3);
	bool range_max = param_is("color_range_max", &max, sizeof(float) * 3);
	end_technique(tech);
	gs_leave_context();

	assert_no_line();
	assert_int_equal(sizeof(m), 64);
	assert_true(matrix);
	assert_true(range_min);
	assert_true(range_max);
}

/* A NULL min is (0, 0, 0) and a NULL max (1, 1, 1), each on its own. */
static void color_matrix_null_ranges_default(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	const float zero[3] = {0.0f, 0.0f, 0.0f};
	const float one[3] = {1.0f, 1.0f, 1.0f};
	struct matrix4 m;
	struct vec3 sentinel, min, max;
	fill_matrix(&m, 7.0f);
	vec3_set(&sentinel, 0.375f, 0.375f, 0.375f);
	vec3_set(&min, 0.0625f, 0.125f, 0.25f);
	vec3_set(&max, 0.5f, 0.75f, 0.9375f);

	struct {
		const struct vec3 *min;
		const struct vec3 *max;
		const void *want_min;
		const void *want_max;
	} cases[] = {
		{NULL, NULL, zero, one},
		{NULL, &max, zero, &max},
		{&min, NULL, &min, one},
	};

	int bad = 0;
	gs_enter_context(graphics);
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		gs_technique_t *tech = begin_technique();
		gs_effect_set_val(param("color_range_min"), &sentinel, sizeof(float) * 3);
		gs_effect_set_val(param("color_range_max"), &sentinel, sizeof(float) * 3);
		start_log();
		obs_source_draw_set_color_matrix(&m, cases[i].min, cases[i].max);
		stop_log();
		if (line_count != 0 || !param_is("color_matrix", &m, sizeof(m)) ||
		    !param_is("color_range_min", cases[i].want_min, sizeof(float) * 3) ||
		    !param_is("color_range_max", cases[i].want_max, sizeof(float) * 3)) {
			fprintf(stderr, "case %zu: min %s, max %s: wrong values or a log line\n", i,
				cases[i].min ? "given" : "NULL", cases[i].max ? "given" : "NULL");
			bad++;
		}
		end_technique(tech);
	}
	gs_leave_context();

	assert_int_equal(bad, 0);
}

/* ------------------------------------------------------------------------- */
/* obs_source_draw */

struct target {
	uint8_t px[SIZE][SIZE][4]; /* [row][column] */
};

struct draw_result {
	struct target target;
	struct gs_shader_texture image;
	size_t image_size;
	bool srgb_after;
	struct matrix4 matrix_before;
	struct matrix4 matrix_after;
};

enum how {
	IN_TECHNIQUE, /* inside a pass of the Draw technique */
	NO_EFFECT,    /* with no current effect */
};

/* Clears a SIZE x SIZE target to transparent black, sets the framebuffer
 * sRGB flag and linear sRGB, calls obs_source_draw() as told and reads the
 * target back, with the image param and the flag and matrix after the
 * call. */
static void draw(enum how how, gs_texture_t *tex, int x, int y, uint32_t cx, uint32_t cy, bool flip, bool srgb_before,
		 bool linear_srgb, struct draw_result *out)
{
	memset(out, 0, sizeof(*out));
	gs_enter_context(graphics);
	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	gs_stagesurf_t *stage = gs_stagesurface_create(SIZE, SIZE, GS_RGBA);
	assert_non_null(tr);
	assert_non_null(stage);

	assert_true(gs_texrender_begin(tr, SIZE, SIZE));
	gs_ortho(0.0f, (float)SIZE, 0.0f, (float)SIZE, -100.0f, 100.0f);
	gs_matrix_push();
	gs_matrix_identity();
	/* as the core's render loop sets them (no core runs here) */
	gs_set_cull_mode(GS_NEITHER);
	gs_enable_depth_test(false);

	struct vec4 clear;
	vec4_set(&clear, 0.0f, 0.0f, 0.0f, 0.0f);
	gs_clear(GS_CLEAR_COLOR, &clear, 0.0f, 0);

	gs_blend_state_push();
	gs_enable_blending(false);

	const bool srgb_outer = gs_framebuffer_srgb_enabled();
	const bool linear_outer = gs_get_linear_srgb();
	gs_enable_framebuffer_srgb(srgb_before);
	gs_set_linear_srgb(linear_srgb);

	if (how == NO_EFFECT) {
		assert_null(gs_get_effect());
		gs_matrix_get(&out->matrix_before);
		obs_source_draw(tex, x, y, cx, cy, flip);
		gs_matrix_get(&out->matrix_after);
		out->srgb_after = gs_framebuffer_srgb_enabled();
	} else {
		while (gs_effect_loop(effect, "Draw")) {
			gs_eparam_t *image = param("image");
			gs_matrix_get(&out->matrix_before);
			obs_source_draw(tex, x, y, cx, cy, flip);
			gs_matrix_get(&out->matrix_after);
			out->srgb_after = gs_framebuffer_srgb_enabled();
			out->image_size = gs_effect_get_val_size(image);
			if (out->image_size == sizeof(out->image)) {
				void *val = gs_effect_get_val(image);
				memcpy(&out->image, val, sizeof(out->image));
				bfree(val);
			}
		}
	}

	gs_set_linear_srgb(linear_outer);
	gs_enable_framebuffer_srgb(srgb_outer);
	gs_blend_state_pop();
	gs_matrix_pop();
	gs_texrender_end(tr);

	gs_stage_texture(stage, gs_texrender_get_texture(tr));
	uint8_t *mapped;
	uint32_t linesize;
	assert_true(gs_stagesurface_map(stage, &mapped, &linesize));
	for (int row = 0; row < SIZE; row++)
		memcpy(out->target.px[row], mapped + (size_t)row * linesize, SIZE * 4);
	gs_stagesurface_unmap(stage);

	gs_stagesurface_destroy(stage);
	gs_texrender_destroy(tr);
	gs_leave_context();
}

/* The texture drawn at (x, y), w x h (scaled from 2 x 2 by point sampling),
 * upside down with flip, over transparent black. */
static void expect(int x, int y, int w, int h, bool flip, struct target *want)
{
	memset(want, 0, sizeof(*want));
	for (int row = 0; row < SIZE; row++) {
		for (int col = 0; col < SIZE; col++) {
			int u = col - x, v = row - y;
			if (u < 0 || v < 0 || u >= w || v >= h)
				continue;
			int tu = u * 2 / w;
			int tv = v * 2 / h;
			if (flip)
				tv = 1 - tv;
			memcpy(want->px[row][col], texels[tv][tu], 4);
		}
	}
}

static int target_mismatches(const struct target *got, const struct target *want, const char *what)
{
	int bad = 0;
	for (int row = 0; row < SIZE; row++) {
		for (int col = 0; col < SIZE; col++) {
			for (int c = 0; c < 4; c++) {
				int d = (int)got->px[row][col][c] - (int)want->px[row][col][c];
				if (d < -TOLERANCE || d > TOLERANCE) {
					if (bad < 4)
						fprintf(stderr,
							"%s: pixel (%d, %d) RGBA %u %u %u %u, want %u %u %u %u\n", what,
							col, row, got->px[row][col][0], got->px[row][col][1],
							got->px[row][col][2], got->px[row][col][3],
							want->px[row][col][0], want->px[row][col][1],
							want->px[row][col][2], want->px[row][col][3]);
					bad++;
					break;
				}
			}
		}
	}
	return bad;
}

static bool matrix_same(const struct matrix4 *a, const struct matrix4 *b)
{
	return memcmp(a, b, sizeof(*a)) == 0;
}

/* A NULL texture: obs_ptr_valid()'s debug line and nothing drawn, with an
 * effect and without one (the texture is checked first). */
static void draw_null_texture_draws_nothing(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	struct target empty;
	memset(&empty, 0, sizeof(empty));
	struct draw_result r;

	start_log();
	draw(IN_TECHNIQUE, NULL, 1, 1, 0, 0, false, false, true, &r);
	stop_log();
	assert_one_line(LOG_DEBUG, "obs_source_draw: Null 'texture' parameter");
	assert_int_equal(target_mismatches(&r.target, &empty, "NULL texture"), 0);
	assert_int_equal(r.image_size, 0);
	assert_true(matrix_same(&r.matrix_before, &r.matrix_after));

	start_log();
	draw(NO_EFFECT, NULL, 1, 1, 0, 0, false, true, false, &r);
	stop_log();
	assert_one_line(LOG_DEBUG, "obs_source_draw: Null 'texture' parameter");
	assert_int_equal(target_mismatches(&r.target, &empty, "NULL texture, no effect"), 0);
	assert_true(r.srgb_after);
}

/* No current effect: one warning, nothing drawn, the sRGB flag and the
 * matrix as they were. */
static void draw_without_effect_warns(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	struct target empty;
	memset(&empty, 0, sizeof(empty));
	int bad = 0;
	for (int s = 0; s < 4; s++) {
		const bool srgb = (s & 1) != 0, linear = (s & 2) != 0;
		struct draw_result r;
		start_log();
		draw(NO_EFFECT, texture, 2, 3, 0, 0, true, srgb, linear, &r);
		stop_log();
		assert_one_line(LOG_WARNING, "obs_source_draw: no active effect!");
		bad += target_mismatches(&r.target, &empty, "no effect");
		if (r.srgb_after != srgb || !matrix_same(&r.matrix_before, &r.matrix_after)) {
			fprintf(stderr, "no effect, sRGB %d: flag or matrix changed\n", srgb);
			bad++;
		}
	}
	assert_int_equal(bad, 0);
}

/* "image" is the texture, marked sRGB exactly when linear sRGB is on; the
 * framebuffer sRGB flag and the matrix come back as they were, for each
 * flag and linear sRGB, at the origin and off it; and nothing is logged. */
static void draw_sets_image_and_restores_state(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	int bad = 0;
	for (int s = 0; s < 8; s++) {
		const bool srgb = (s & 1) != 0, linear = (s & 2) != 0, offset = (s & 4) != 0;
		const int x = offset ? 3 : 0, y = offset ? 1 : 0;
		struct draw_result r;
		start_log();
		draw(IN_TECHNIQUE, texture, x, y, 0, 0, false, srgb, linear, &r);
		stop_log();
		if (line_count != 0 || r.image_size != sizeof(struct gs_shader_texture) || r.image.tex != texture ||
		    r.image.srgb != linear || r.srgb_after != srgb || !matrix_same(&r.matrix_before, &r.matrix_after)) {
			fprintf(stderr,
				"sRGB %d, linear %d, at (%d, %d): %d lines, image size %zu tex %s srgb %d, "
				"flag after %d, matrix %s\n",
				srgb, linear, x, y, line_count, r.image_size, r.image.tex == texture ? "ok" : "wrong",
				r.image.srgb, r.srgb_after,
				matrix_same(&r.matrix_before, &r.matrix_after) ? "kept" : "changed");
			bad++;
		}
	}
	assert_int_equal(bad, 0);
}

/* The texture lands at (x, y), its own size for cx = cy = 0, else cx by cy,
 * upside down with flip, with linear sRGB off and on. */
static void draw_places_and_flips(void **state)
{
	(void)state;
	if (!graphics)
		skip();

	struct {
		int x, y;
		uint32_t cx, cy;
		bool flip;
	} cases[] = {
		{0, 0, 0, 0, false}, {0, 0, 0, 0, true}, {3, 2, 0, 0, false},  {3, 2, 0, 0, true},
		{5, 0, 0, 0, false}, {0, 6, 0, 0, true}, {-1, 0, 0, 0, false}, {0, -1, 0, 0, true},
		{2, 2, 4, 4, false}, {2, 2, 4, 4, true}, {0, 0, 4, 2, true},   {1, 3, 2, 4, false},
	};

	int bad = 0;
	for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
		for (int l = 0; l < 2; l++) {
			struct draw_result r;
			draw(IN_TECHNIQUE, texture, cases[i].x, cases[i].y, cases[i].cx, cases[i].cy, cases[i].flip,
			     false, l == 1, &r);
			const int w = cases[i].cx ? (int)cases[i].cx : 2;
			const int h = cases[i].cy ? (int)cases[i].cy : 2;
			struct target want;
			expect(cases[i].x, cases[i].y, w, h, cases[i].flip, &want);
			char what[96];
			snprintf(what, sizeof(what), "(%d, %d) %ux%u flip %d linear %d", cases[i].x, cases[i].y,
				 cases[i].cx, cases[i].cy, cases[i].flip, l);
			bad += target_mismatches(&r.target, &want, what);
		}
	}
	assert_int_equal(bad, 0);
}

/* ------------------------------------------------------------------------- */

static bool display_set(void)
{
	return getenv("DISPLAY") || getenv("WAYLAND_DISPLAY");
}

static int group_setup(void **state)
{
	(void)state;
	int ret = gs_create(&graphics, GRAPHICS_MODULE_PATH, 0);
	if (ret != GS_SUCCESS) {
		graphics = NULL;
		if (display_set()) {
			fprintf(stderr, "gs_create failed with a display set: %d\n", ret);
			return -1;
		}
		fprintf(stderr, "skipped: gs_create failed (no display?)\n");
		return 0;
	}

	gs_enter_context(graphics);
	char *errors = NULL;
	effect = gs_effect_create(effect_text, NULL, &errors);
	if (!effect)
		fprintf(stderr, "gs_effect_create failed: %s\n", errors ? errors : "(no errors)");
	bfree(errors);
	const uint8_t *planes[] = {&texels[0][0][0]};
	texture = gs_texture_create(2, 2, GS_RGBA, 1, planes, 0);
	gs_leave_context();

	if (!effect || !texture) {
		fprintf(stderr, "effect or texture not created\n");
		return -1;
	}
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (graphics) {
		gs_enter_context(graphics);
		gs_texture_destroy(texture);
		gs_effect_destroy(effect);
		gs_leave_context();
		gs_destroy(graphics);
	}
	texture = NULL;
	effect = NULL;
	graphics = NULL;
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(color_matrix_without_effect_warns),
		cmocka_unit_test(color_matrix_null_matrix_sets_nothing),
		cmocka_unit_test(color_matrix_sets_matrix_and_ranges),
		cmocka_unit_test(color_matrix_null_ranges_default),
		cmocka_unit_test(draw_null_texture_draws_nothing),
		cmocka_unit_test(draw_without_effect_warns),
		cmocka_unit_test(draw_sets_image_and_restores_state),
		cmocka_unit_test(draw_places_and_flips),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
