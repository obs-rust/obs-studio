#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <limits.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "graphics/graphics-sprite.h"

/*
 * Characterization of the sprite and texture upload geometry of
 * libobs/graphics/graphics.c, moved to libobs/graphics/graphics-sprite.c
 * before its Rust port:
 * - the four points and texture coordinates of a sprite (build_sprite()),
 *   normalized (build_sprite_norm(), the static sprite buffers), in texels
 *   for a rectangle texture (build_sprite_rect()) and for a subregion
 *   (build_subsprite_norm());
 * - gs_draw_quadf(): the refusals, the size taken from the texture, and the
 *   static buffer a flip draws;
 * - gs_draw_sprite_subregion(): the "whole texture" shortcut;
 * - gs_texture_set_image(): the copy plan;
 * - gs_texture_create(): is_pow2() and the mipmap rules.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them (test_sprite_geometry_steps_rust runs it
 * against the Rust shims). Floats are compared by their bits, except that a
 * NaN is only asked to be a NaN: which NaN 0 / 0 gives is the host's.
 * test_texture_render, test_image_size_precheck, the test_graphics_alias_*
 * tests and the scene render tests cover the callers through the real
 * libobs.
 */

#define FLIP_U GS_SPRITE_FLIP_U
#define FLIP_V GS_SPRITE_FLIP_V
#define BUILD_MIPMAPS GS_SPRITE_BUILD_MIPMAPS
#define RENDER_TARGET GS_SPRITE_RENDER_TARGET
/* GS_DYNAMIC and GS_SHARED_TEX of graphics.h: flags the mipmap rules keep */
#define OTHER_FLAGS ((1u << 1) | (1u << 5))

static uint32_t bits(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

static float from_bits(uint32_t u)
{
	float f;
	memcpy(&f, &u, sizeof(f));
	return f;
}

/* The same float, bit for bit (+0 and -0 differ), or both NaN. */
static void check_float(float actual, float expected, int line)
{
	if (isnan(expected)) {
		if (!isnan(actual)) {
			fprintf(stderr, "line %d: %a is not a NaN\n", line, (double)actual);
			fail();
		}
		return;
	}
	if (bits(actual) != bits(expected)) {
		fprintf(stderr, "line %d: %a (0x%08x), expected %a (0x%08x)\n", line, (double)actual, bits(actual),
			(double)expected, bits(expected));
		fail();
	}
}

#define assert_float_bits(actual, expected) check_float((actual), (expected), __LINE__)

static void check_uv(struct gs_sprite_uv uv, float su, float eu, float sv, float ev, int line)
{
	check_float(uv.start_u, su, line);
	check_float(uv.end_u, eu, line);
	check_float(uv.start_v, sv, line);
	check_float(uv.end_v, ev, line);
}

#define assert_uv(uv, su, eu, sv, ev) check_uv((uv), (su), (eu), (sv), (ev), __LINE__)

/* Every flip value: none, U, V, both, and with other bits set. */
static const uint32_t flips[] = {
	0,
	FLIP_U,
	FLIP_V,
	FLIP_U | FLIP_V,
	1u << 2,
	FLIP_U | (1u << 2),
	FLIP_V | (1u << 31),
	FLIP_U | FLIP_V | 0xfffffff0u,
	0xfffffffcu,
	UINT32_MAX,
};

#define FLIP_COUNT (sizeof(flips) / sizeof(flips[0]))

static bool has_u(uint32_t flip)
{
	return (flip & FLIP_U) != 0;
}

static bool has_v(uint32_t flip)
{
	return (flip & FLIP_V) != 0;
}

/* build_sprite(): the points, z and the padding lane cleared, and the
 * texture coordinates in triangle-strip order. Every lane of both arrays is
 * written, whatever was there. */
static void build_writes_every_lane(void **state)
{
	(void)state;
	struct vec3 points[4];
	struct vec2 uvs[4];
	struct gs_sprite_uv uv = {0.25f, 0.75f, 0.125f, 0.875f};

	memset(points, 0x7f, sizeof(points));
	memset(uvs, 0x7f, sizeof(uvs));
	gs_sprite_build(points, uvs, 1920.0f, 1080.0f, uv);

	const float expected_points[4][4] = {
		{0.0f, 0.0f, 0.0f, 0.0f},
		{1920.0f, 0.0f, 0.0f, 0.0f},
		{0.0f, 1080.0f, 0.0f, 0.0f},
		{1920.0f, 1080.0f, 0.0f, 0.0f},
	};
	const float expected_uvs[4][2] = {
		{0.25f, 0.125f},
		{0.75f, 0.125f},
		{0.25f, 0.875f},
		{0.75f, 0.875f},
	};
	for (size_t i = 0; i < 4; i++) {
		assert_float_bits(points[i].x, expected_points[i][0]);
		assert_float_bits(points[i].y, expected_points[i][1]);
		assert_float_bits(points[i].z, expected_points[i][2]);
		assert_float_bits(points[i].w, expected_points[i][3]);
		assert_float_bits(uvs[i].x, expected_uvs[i][0]);
		assert_float_bits(uvs[i].y, expected_uvs[i][1]);
	}
}

/* The sizes and coordinates go through as they are: -0.0, infinities, NaN
 * payloads and denormals keep their bits. */
static void build_keeps_bits(void **state)
{
	(void)state;
	struct vec3 points[4];
	struct vec2 uvs[4];
	const float nan_payload = from_bits(0x7fc12345u);
	const float denormal = from_bits(0x00000001u);
	struct gs_sprite_uv uv = {-0.0f, INFINITY, nan_payload, denormal};

	memset(points, 0x55, sizeof(points));
	memset(uvs, 0x55, sizeof(uvs));
	gs_sprite_build(points, uvs, -0.0f, -INFINITY, uv);

	assert_int_equal(bits(points[0].x), 0);
	assert_int_equal(bits(points[0].y), 0);
	assert_int_equal(bits(points[1].x), bits(-0.0f));
	assert_int_equal(bits(points[1].y), 0);
	assert_int_equal(bits(points[2].x), 0);
	assert_int_equal(bits(points[2].y), bits(-INFINITY));
	assert_int_equal(bits(points[3].x), bits(-0.0f));
	assert_int_equal(bits(points[3].y), bits(-INFINITY));
	for (size_t i = 0; i < 4; i++) {
		assert_int_equal(bits(points[i].z), 0);
		assert_int_equal(bits(points[i].w), 0);
	}
	assert_int_equal(bits(uvs[0].x), bits(-0.0f));
	assert_int_equal(bits(uvs[0].y), 0x7fc12345u);
	assert_int_equal(bits(uvs[1].x), bits(INFINITY));
	assert_int_equal(bits(uvs[1].y), 0x7fc12345u);
	assert_int_equal(bits(uvs[2].x), bits(-0.0f));
	assert_int_equal(bits(uvs[2].y), 0x00000001u);
	assert_int_equal(bits(uvs[3].x), bits(INFINITY));
	assert_int_equal(bits(uvs[3].y), 0x00000001u);
}

/* build_sprite_norm(): 0 to 1, each axis swapped by its own bit; every
 * other bit is ignored. */
static void uv_norm_each_flip(void **state)
{
	(void)state;
	for (size_t i = 0; i < FLIP_COUNT; i++) {
		uint32_t flip = flips[i];
		struct gs_sprite_uv uv = gs_sprite_uv_norm(flip);

		assert_uv(uv, has_u(flip) ? 1.0f : 0.0f, has_u(flip) ? 0.0f : 1.0f, has_v(flip) ? 1.0f : 0.0f,
			  has_v(flip) ? 0.0f : 1.0f);
	}
	assert_uv(gs_sprite_uv_norm(0), 0.0f, 1.0f, 0.0f, 1.0f);
	assert_uv(gs_sprite_uv_norm(FLIP_U), 1.0f, 0.0f, 0.0f, 1.0f);
	assert_uv(gs_sprite_uv_norm(FLIP_V), 0.0f, 1.0f, 1.0f, 0.0f);
	assert_uv(gs_sprite_uv_norm(FLIP_U | FLIP_V), 1.0f, 0.0f, 1.0f, 0.0f);
	assert_uv(gs_sprite_uv_norm(1u << 2), 0.0f, 1.0f, 0.0f, 1.0f);
}

/* The static buffers graphics_init_sprite_vbs() builds: the plain sprite
 * and the one flipped on v, both 1 by 1 (gs_draw_quadf() scales them). */
static void static_sprite_buffers(void **state)
{
	(void)state;
	struct vec3 points[4];
	struct vec2 uvs[4];

	gs_sprite_build(points, uvs, 1.0f, 1.0f, gs_sprite_uv_norm(0));
	const float plain[4][2] = {{0, 0}, {1, 0}, {0, 1}, {1, 1}};
	for (size_t i = 0; i < 4; i++) {
		assert_float_bits(points[i].x, plain[i][0]);
		assert_float_bits(points[i].y, plain[i][1]);
		assert_float_bits(uvs[i].x, plain[i][0]);
		assert_float_bits(uvs[i].y, plain[i][1]);
	}

	gs_sprite_build(points, uvs, 1.0f, 1.0f, gs_sprite_uv_norm(FLIP_V));
	const float flipped[4][2] = {{0, 1}, {1, 1}, {0, 0}, {1, 0}};
	for (size_t i = 0; i < 4; i++) {
		assert_float_bits(points[i].x, plain[i][0]);
		assert_float_bits(points[i].y, plain[i][1]);
		assert_float_bits(uvs[i].x, flipped[i][0]);
		assert_float_bits(uvs[i].y, flipped[i][1]);
	}
}

/* build_sprite_rect(): 0 to the texture's size in texels, the size
 * converted to float (rounded to nearest even past 2^24). */
static void uv_rect_each_flip(void **state)
{
	(void)state;
	const uint32_t sizes[][2] = {
		{1920, 1080}, {0, 0}, {1, 0}, {16777217, 16777219}, {UINT32_MAX, 0x80000001u},
	};
	const float fsizes[][2] = {
		{1920.0f, 1080.0f},
		{0.0f, 0.0f},
		{1.0f, 0.0f},
		{16777216.0f, 16777220.0f},
		{4294967296.0f, 2147483648.0f},
	};

	for (size_t s = 0; s < sizeof(sizes) / sizeof(sizes[0]); s++) {
		for (size_t i = 0; i < FLIP_COUNT; i++) {
			uint32_t flip = flips[i];
			float w = fsizes[s][0];
			float h = fsizes[s][1];
			struct gs_sprite_uv uv = gs_sprite_uv_rect(sizes[s][0], sizes[s][1], flip);

			assert_uv(uv, has_u(flip) ? w : 0.0f, has_u(flip) ? 0.0f : w, has_v(flip) ? h : 0.0f,
				  has_v(flip) ? 0.0f : h);
		}
	}
}

/* build_subsprite_norm(): a subregion inside the texture, a quarter in from
 * each side. */
static void uv_subregion_inside(void **state)
{
	(void)state;
	for (size_t i = 0; i < FLIP_COUNT; i++) {
		uint32_t flip = flips[i];
		struct gs_sprite_uv uv = gs_sprite_uv_subregion(480, 270, 960, 540, 1920, 1080, flip);

		assert_uv(uv, has_u(flip) ? 0.75f : 0.25f, has_u(flip) ? 0.25f : 0.75f, has_v(flip) ? 0.75f : 0.25f,
			  has_v(flip) ? 0.25f : 0.75f);
	}
	/* not exact: 1 / 3 and 2 / 3 rounded in float */
	struct gs_sprite_uv uv = gs_sprite_uv_subregion(1, 1, 1, 1, 3, 3, 0);
	assert_uv(uv, 1.0f / 3.0f, 2.0f / 3.0f, 1.0f / 3.0f, 2.0f / 3.0f);
	assert_int_equal(bits(uv.start_u), 0x3eaaaaabu);
	assert_int_equal(bits(uv.end_u), 0x3f2aaaabu);
}

/* At the edges: the whole texture, a 1-texel column and row at each side,
 * and an empty region at the far corner. */
static void uv_subregion_edges(void **state)
{
	(void)state;
	assert_uv(gs_sprite_uv_subregion(0, 0, 1920, 1080, 1920, 1080, 0), 0.0f, 1.0f, 0.0f, 1.0f);
	assert_uv(gs_sprite_uv_subregion(0, 0, 1920, 1080, 1920, 1080, FLIP_U | FLIP_V), 1.0f, 0.0f, 1.0f, 0.0f);
	assert_uv(gs_sprite_uv_subregion(0, 0, 1, 1, 4, 2, 0), 0.0f, 0.25f, 0.0f, 0.5f);
	assert_uv(gs_sprite_uv_subregion(3, 1, 1, 1, 4, 2, 0), 0.75f, 1.0f, 0.5f, 1.0f);
	assert_uv(gs_sprite_uv_subregion(3, 1, 1, 1, 4, 2, FLIP_U), 1.0f, 0.75f, 0.5f, 1.0f);
	assert_uv(gs_sprite_uv_subregion(3, 1, 1, 1, 4, 2, FLIP_V), 0.75f, 1.0f, 1.0f, 0.5f);
	assert_uv(gs_sprite_uv_subregion(4, 2, 0, 0, 4, 2, 0), 1.0f, 1.0f, 1.0f, 1.0f);
	assert_uv(gs_sprite_uv_subregion(0, 0, 0, 0, 4, 2, FLIP_U | FLIP_V), 0.0f, 0.0f, 0.0f, 0.0f);
}

/* Larger than the texture, or past it: coordinates above 1, which the
 * sampler's address mode decides. Characterized, not endorsed: nothing is
 * clamped, and the sum x + cx is a float sum, so past 2^24 it rounds. */
static void uv_subregion_larger_than_texture(void **state)
{
	(void)state;
	assert_uv(gs_sprite_uv_subregion(0, 0, 3840, 2160, 1920, 1080, 0), 0.0f, 2.0f, 0.0f, 2.0f);
	assert_uv(gs_sprite_uv_subregion(1920, 1080, 960, 540, 1920, 1080, 0), 1.0f, 1.5f, 1.0f, 1.5f);
	assert_uv(gs_sprite_uv_subregion(1920, 1080, 960, 540, 1920, 1080, FLIP_U | FLIP_V), 1.5f, 1.0f, 1.5f, 1.0f);
	/* UINT32_MAX is 2^32 in float */
	assert_uv(gs_sprite_uv_subregion(UINT32_MAX, 0, UINT32_MAX, UINT32_MAX, 1, 1, 0), 4294967296.0f, 8589934592.0f,
		  0.0f, 4294967296.0f);
	/* 16777216 + 1 is 16777216 in float: the region has no width */
	struct gs_sprite_uv uv = gs_sprite_uv_subregion(16777216, 0, 1, 1, 16777216, 1, 0);
	assert_uv(uv, 1.0f, 1.0f, 0.0f, 1.0f);
}

/* A texture of size 0 (gs_texture_get_width() of a NULL texture is 0):
 * division by 0. Characterized, not endorsed: x / 0 is +inf, 0 / 0 is NaN,
 * and both reach the vertex buffer. */
static void uv_subregion_zero_size_texture(void **state)
{
	(void)state;
	assert_uv(gs_sprite_uv_subregion(0, 0, 0, 0, 0, 0, 0), NAN, NAN, NAN, NAN);
	assert_uv(gs_sprite_uv_subregion(0, 0, 10, 20, 0, 0, 0), NAN, INFINITY, NAN, INFINITY);
	assert_uv(gs_sprite_uv_subregion(0, 0, 10, 20, 0, 0, FLIP_U | FLIP_V), INFINITY, NAN, INFINITY, NAN);
	assert_uv(gs_sprite_uv_subregion(5, 6, 10, 20, 0, 0, FLIP_U), INFINITY, INFINITY, INFINITY, INFINITY);
	/* one axis of size 0 only */
	assert_uv(gs_sprite_uv_subregion(1, 0, 1, 1, 2, 0, 0), 0.5f, 1.0f, NAN, INFINITY);
	assert_uv(gs_sprite_uv_subregion(0, 1, 1, 1, 0, 2, FLIP_V), NAN, INFINITY, 1.0f, 0.5f);
}

/* gs_draw_sprite_subregion() draws a plain sprite only for (0, 0) and the
 * texture's own size. */
static void subregion_whole_shortcut(void **state)
{
	(void)state;
	assert_true(gs_sprite_subregion_is_whole(0, 0, 1920, 1080, 1920, 1080));
	assert_false(gs_sprite_subregion_is_whole(1, 0, 1920, 1080, 1920, 1080));
	assert_false(gs_sprite_subregion_is_whole(0, 1, 1920, 1080, 1920, 1080));
	assert_false(gs_sprite_subregion_is_whole(0, 0, 1919, 1080, 1920, 1080));
	assert_false(gs_sprite_subregion_is_whole(0, 0, 1920, 1079, 1920, 1080));
	assert_false(gs_sprite_subregion_is_whole(0, 0, 1921, 1080, 1920, 1080));
	assert_false(gs_sprite_subregion_is_whole(0, 0, 1080, 1920, 1920, 1080));
	assert_true(gs_sprite_subregion_is_whole(0, 0, UINT32_MAX, UINT32_MAX, UINT32_MAX, UINT32_MAX));
	/* A NULL texture is 0 by 0: a 0-size region at (0, 0) is "whole", so
	 * gs_draw_sprite(NULL, flip, 0, 0) refuses it with a log line. */
	assert_true(gs_sprite_subregion_is_whole(0, 0, 0, 0, 0, 0));
	assert_false(gs_sprite_subregion_is_whole(0, 0, 1, 0, 0, 0));
}

/* gs_draw_quadf(): a texture must be 2D, whatever the size; without one
 * neither size may be 0. -0.0 is 0; NaN, infinities and denormals are
 * sizes. */
static void quad_refusals(void **state)
{
	(void)state;
	const float sizes[] = {0.0f, -0.0f, 1.0f, -1.0f, INFINITY, -INFINITY, NAN, 1e-45f, 1920.0f};
	const size_t count = sizeof(sizes) / sizeof(sizes[0]);

	for (size_t w = 0; w < count; w++) {
		for (size_t h = 0; h < count; h++) {
			float width = sizes[w];
			float height = sizes[h];
			bool zero = width == 0.0f || height == 0.0f;

			assert_int_equal(gs_sprite_quad_refusal(true, true, width, height), GS_SPRITE_QUAD_DRAW);
			assert_int_equal(gs_sprite_quad_refusal(true, false, width, height), GS_SPRITE_QUAD_NOT_2D);
			assert_int_equal(gs_sprite_quad_refusal(false, true, width, height),
					 zero ? GS_SPRITE_QUAD_NO_SIZE : GS_SPRITE_QUAD_DRAW);
			assert_int_equal(gs_sprite_quad_refusal(false, false, width, height),
					 zero ? GS_SPRITE_QUAD_NO_SIZE : GS_SPRITE_QUAD_DRAW);
		}
	}
	assert_int_equal(gs_sprite_quad_refusal(false, false, 0.0f, 1.0f), GS_SPRITE_QUAD_NO_SIZE);
	assert_int_equal(gs_sprite_quad_refusal(false, false, 1.0f, -0.0f), GS_SPRITE_QUAD_NO_SIZE);
	/* characterized, not endorsed: a NaN or negative size draws */
	assert_int_equal(gs_sprite_quad_refusal(false, false, NAN, -1.0f), GS_SPRITE_QUAD_DRAW);
}

/* An axis takes the texture's size only for 0 or -0.0. */
static void quad_texture_size(void **state)
{
	(void)state;
	assert_true(gs_sprite_quad_texture_size(0.0f));
	assert_true(gs_sprite_quad_texture_size(-0.0f));
	assert_false(gs_sprite_quad_texture_size(1e-45f));
	assert_false(gs_sprite_quad_texture_size(-1e-45f));
	assert_false(gs_sprite_quad_texture_size(1.0f));
	assert_false(gs_sprite_quad_texture_size(-1.0f));
	assert_false(gs_sprite_quad_texture_size(INFINITY));
	assert_false(gs_sprite_quad_texture_size(NAN));
	assert_false(gs_sprite_quad_texture_size(from_bits(0xffc00000u)));
}

/* The static buffer gs_draw_quadf() draws for a texture that is not a
 * rectangle texture: the flipped one for any bit set. Characterized, not
 * endorsed: that buffer is flipped on v only (static_sprite_buffers), so
 * GS_FLIP_U alone draws the texture upside down instead of mirrored, and
 * GS_FLIP_U | GS_FLIP_V is not turned on u. Since upstream 7b8b51b29
 * ("Improve sprite drawing perf"); a subregion or a rectangle texture still
 * honours each bit. */
static void quad_flipped_buffer(void **state)
{
	(void)state;
	assert_false(gs_sprite_quad_flipped_buffer(0));
	for (size_t i = 1; i < FLIP_COUNT; i++)
		assert_true(gs_sprite_quad_flipped_buffer(flips[i]));
	assert_true(gs_sprite_quad_flipped_buffer(FLIP_U));
	assert_true(gs_sprite_quad_flipped_buffer(1u << 2));
}

static void check_plan(struct gs_texture_upload_plan plan, enum gs_texture_upload_mode mode, size_t row_copy,
		       size_t whole_size, size_t src_offset, int line)
{
	if (plan.mode != mode || plan.row_copy != row_copy || plan.whole_size != whole_size ||
	    plan.src_offset != src_offset) {
		fprintf(stderr,
			"line %d: mode %d row_copy %zu whole_size %zu src_offset %zu, expected %d %zu %zu %zu\n", line,
			(int)plan.mode, plan.row_copy, plan.whole_size, plan.src_offset, (int)mode, row_copy,
			whole_size, src_offset);
		fail();
	}
}

#define assert_plan(plan, mode, row_copy, whole_size, src_offset) \
	check_plan((plan), (mode), (row_copy), (whole_size), (src_offset), __LINE__)

/* Equal line sizes: one copy of the whole image. */
static void upload_equal_linesizes(void **state)
{
	(void)state;
	assert_plan(gs_texture_upload_plan(7680, 7680, 1080, false), GS_TEXTURE_UPLOAD_WHOLE, 7680, 7680 * 1080, 0);
	assert_plan(gs_texture_upload_plan(4, 4, 1, false), GS_TEXTURE_UPLOAD_WHOLE, 4, 4, 0);
	/* a height of 0 copies 0 bytes */
	assert_plan(gs_texture_upload_plan(7680, 7680, 0, false), GS_TEXTURE_UPLOAD_WHOLE, 7680, 0, 0);
	assert_plan(gs_texture_upload_plan(0, 0, 1080, false), GS_TEXTURE_UPLOAD_WHOLE, 0, 0, 0);
	/* size_t arithmetic, wrapping where size_t does */
	assert_plan(gs_texture_upload_plan(UINT32_MAX, UINT32_MAX, UINT32_MAX, false), GS_TEXTURE_UPLOAD_WHOLE,
		    UINT32_MAX, (size_t)UINT32_MAX * (size_t)UINT32_MAX, 0);
}

/* Unequal line sizes: row by row, the shorter line's bytes each. */
static void upload_unequal_linesizes(void **state)
{
	(void)state;
	assert_plan(gs_texture_upload_plan(7680, 8192, 1080, false), GS_TEXTURE_UPLOAD_ROWS, 7680, 0, 0);
	assert_plan(gs_texture_upload_plan(8192, 7680, 1080, false), GS_TEXTURE_UPLOAD_ROWS, 7680, 0, 0);
	assert_plan(gs_texture_upload_plan(0, 64, 4, false), GS_TEXTURE_UPLOAD_ROWS, 0, 0, 0);
	assert_plan(gs_texture_upload_plan(64, 0, 4, false), GS_TEXTURE_UPLOAD_ROWS, 0, 0, 0);
	assert_plan(gs_texture_upload_plan(1, UINT32_MAX, 2, false), GS_TEXTURE_UPLOAD_ROWS, 1, 0, 0);
	/* a height of 0: graphics.c's loop copies no row */
	assert_plan(gs_texture_upload_plan(7680, 8192, 0, false), GS_TEXTURE_UPLOAD_ROWS, 7680, 0, 0);
}

/* With a flip: row by row from the source's last row, whatever the line
 * sizes, even equal ones. */
static void upload_flipped(void **state)
{
	(void)state;
	assert_plan(gs_texture_upload_plan(7680, 7680, 1080, true), GS_TEXTURE_UPLOAD_FLIPPED, 7680, 0,
		    (size_t)1079 * 7680);
	assert_plan(gs_texture_upload_plan(7680, 8192, 1080, true), GS_TEXTURE_UPLOAD_FLIPPED, 7680, 0,
		    (size_t)1079 * 7680);
	assert_plan(gs_texture_upload_plan(8192, 7680, 1080, true), GS_TEXTURE_UPLOAD_FLIPPED, 7680, 0,
		    (size_t)1079 * 8192);
	assert_plan(gs_texture_upload_plan(16, 16, 1, true), GS_TEXTURE_UPLOAD_FLIPPED, 16, 0, 0);
	assert_plan(gs_texture_upload_plan(0, 16, 3, true), GS_TEXTURE_UPLOAD_FLIPPED, 0, 0, 0);
	assert_plan(gs_texture_upload_plan(UINT32_MAX, 1, UINT32_MAX, true), GS_TEXTURE_UPLOAD_FLIPPED, 1, 0,
		    ((size_t)UINT32_MAX - 1) * (size_t)UINT32_MAX);
}

/* A flip with a height of 0 copies nothing, and the plan says so before it
 * computes the start row: graphics.c used to compute (height - 1) *
 * linesize, which wraps, and move the source pointer by it (undefined)
 * before its loop found no row. */
static void upload_flipped_height_zero(void **state)
{
	(void)state;
	assert_plan(gs_texture_upload_plan(7680, 7680, 0, true), GS_TEXTURE_UPLOAD_NONE, 7680, 0, 0);
	assert_plan(gs_texture_upload_plan(7680, 8192, 0, true), GS_TEXTURE_UPLOAD_NONE, 7680, 0, 0);
	assert_plan(gs_texture_upload_plan(0, 0, 0, true), GS_TEXTURE_UPLOAD_NONE, 0, 0, 0);
	assert_plan(gs_texture_upload_plan(UINT32_MAX, UINT32_MAX, 0, true), GS_TEXTURE_UPLOAD_NONE, UINT32_MAX, 0, 0);
}

/* is_pow2(): 2 and up, one bit set. 1 is not. */
static void pow2(void **state)
{
	(void)state;
	assert_false(gs_texture_is_pow2(0));
	assert_false(gs_texture_is_pow2(1));
	for (unsigned shift = 1; shift < 32; shift++) {
		uint32_t size = 1u << shift;

		assert_true(gs_texture_is_pow2(size));
		assert_false(gs_texture_is_pow2(size + 1));
		if (size > 2)
			assert_false(gs_texture_is_pow2(size - 1));
		assert_false(gs_texture_is_pow2(size | 1u));
	}
	assert_false(gs_texture_is_pow2(3));
	assert_false(gs_texture_is_pow2(1080));
	assert_false(gs_texture_is_pow2(0x80000001u));
	assert_false(gs_texture_is_pow2(UINT32_MAX));
}

struct mip_size {
	uint32_t width;
	uint32_t height;
	bool pow2;
};

static const struct mip_size mip_sizes[] = {
	{256, 256, true},  {2, 4096, true}, {1u << 31, 2, true}, {1920, 1080, false}, {256, 255, false},
	{255, 256, false}, {1, 1, false},   {0, 0, false},       {1, 256, false},     {256, 0, false},
};

#define MIP_SIZE_COUNT (sizeof(mip_sizes) / sizeof(mip_sizes[0]))

/* gs_texture_create(): mipmaps are on with GS_BUILD_MIPMAPS or levels other
 * than 1. Then a size that is not a power of two warns and turns them off
 * (GS_BUILD_MIPMAPS cleared, 1 level); else GS_RENDER_TARGET does the same
 * with its own warning. At most one warning; every other flag is kept. */
static void mip_rules(void **state)
{
	(void)state;
	const uint32_t levels_list[] = {0, 1, 5};
	const uint32_t flag_sets[] = {0, BUILD_MIPMAPS, RENDER_TARGET, BUILD_MIPMAPS | RENDER_TARGET};

	for (size_t s = 0; s < MIP_SIZE_COUNT; s++) {
		for (size_t l = 0; l < 3; l++) {
			for (size_t f = 0; f < 4; f++) {
				for (int other = 0; other < 2; other++) {
					const struct mip_size *size = &mip_sizes[s];
					uint32_t levels = levels_list[l];
					uint32_t flags = flag_sets[f] | (other ? OTHER_FLAGS : 0);
					bool mipmaps = (flags & BUILD_MIPMAPS) != 0 || levels != 1;
					enum gs_texture_mip_warning warning = GS_TEXTURE_MIP_OK;
					uint32_t want_flags = flags;
					uint32_t want_levels = levels;

					if (mipmaps && !size->pow2)
						warning = GS_TEXTURE_MIP_NOT_POW2;
					else if (mipmaps && (flags & RENDER_TARGET) != 0)
						warning = GS_TEXTURE_MIP_RENDER_TARGET;
					if (warning != GS_TEXTURE_MIP_OK) {
						want_flags &= ~(uint32_t)BUILD_MIPMAPS;
						want_levels = 1;
					}

					struct gs_texture_mip_plan plan =
						gs_texture_mip_plan(size->width, size->height, flags, levels);
					if (plan.warning != warning || plan.flags != want_flags ||
					    plan.levels != want_levels) {
						fprintf(stderr,
							"%ux%u flags 0x%x levels %u: warning %d flags 0x%x levels %u\n",
							size->width, size->height, flags, levels, (int)plan.warning,
							plan.flags, plan.levels);
						fail();
					}
				}
			}
		}
	}
}

/* The same rules, row by row. */
static void mip_rule_rows(void **state)
{
	(void)state;
	struct gs_texture_mip_plan plan;

	/* power of two */
	plan = gs_texture_mip_plan(256, 256, 0, 1);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_OK);
	assert_int_equal(plan.flags, 0);
	assert_int_equal(plan.levels, 1);
	plan = gs_texture_mip_plan(256, 256, 0, 0);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_OK);
	assert_int_equal(plan.levels, 0);
	plan = gs_texture_mip_plan(256, 256, BUILD_MIPMAPS, 5);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_OK);
	assert_int_equal(plan.flags, BUILD_MIPMAPS);
	assert_int_equal(plan.levels, 5);
	/* characterized, not endorsed: GS_BUILD_MIPMAPS with 1 level is kept */
	plan = gs_texture_mip_plan(256, 256, BUILD_MIPMAPS, 1);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_OK);
	assert_int_equal(plan.flags, BUILD_MIPMAPS);
	assert_int_equal(plan.levels, 1);
	plan = gs_texture_mip_plan(256, 256, RENDER_TARGET, 1);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_OK);
	assert_int_equal(plan.flags, RENDER_TARGET);
	plan = gs_texture_mip_plan(256, 256, RENDER_TARGET, 0);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_RENDER_TARGET);
	assert_int_equal(plan.flags, RENDER_TARGET);
	assert_int_equal(plan.levels, 1);
	plan = gs_texture_mip_plan(256, 256, RENDER_TARGET | BUILD_MIPMAPS | OTHER_FLAGS, 5);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_RENDER_TARGET);
	assert_int_equal(plan.flags, RENDER_TARGET | OTHER_FLAGS);
	assert_int_equal(plan.levels, 1);

	/* not a power of two: the size warning wins over the render target's */
	plan = gs_texture_mip_plan(1920, 1080, RENDER_TARGET | BUILD_MIPMAPS, 5);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_NOT_POW2);
	assert_int_equal(plan.flags, RENDER_TARGET);
	assert_int_equal(plan.levels, 1);
	plan = gs_texture_mip_plan(1920, 1080, 0, 0);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_NOT_POW2);
	assert_int_equal(plan.levels, 1);
	plan = gs_texture_mip_plan(1920, 1080, RENDER_TARGET, 1);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_OK);
	assert_int_equal(plan.flags, RENDER_TARGET);
	assert_int_equal(plan.levels, 1);
	/* characterized, not endorsed: a 1x1 texture, whose chain is one level,
	 * warns that it is not a power of two */
	plan = gs_texture_mip_plan(1, 1, 0, 0);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_NOT_POW2);
	assert_int_equal(plan.levels, 1);
	/* one axis is enough */
	plan = gs_texture_mip_plan(256, 255, BUILD_MIPMAPS, 1);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_NOT_POW2);
	assert_int_equal(plan.flags, 0);
	/* every other bit is kept, the unknown ones too */
	plan = gs_texture_mip_plan(300, 300, 0xffffffffu, 9);
	assert_int_equal(plan.warning, GS_TEXTURE_MIP_NOT_POW2);
	assert_int_equal(plan.flags, 0xfffffffeu);
	assert_int_equal(plan.levels, 1);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(build_writes_every_lane),
		cmocka_unit_test(build_keeps_bits),
		cmocka_unit_test(uv_norm_each_flip),
		cmocka_unit_test(static_sprite_buffers),
		cmocka_unit_test(uv_rect_each_flip),
		cmocka_unit_test(uv_subregion_inside),
		cmocka_unit_test(uv_subregion_edges),
		cmocka_unit_test(uv_subregion_larger_than_texture),
		cmocka_unit_test(uv_subregion_zero_size_texture),
		cmocka_unit_test(subregion_whole_shortcut),
		cmocka_unit_test(quad_refusals),
		cmocka_unit_test(quad_texture_size),
		cmocka_unit_test(quad_flipped_buffer),
		cmocka_unit_test(upload_equal_linesizes),
		cmocka_unit_test(upload_unequal_linesizes),
		cmocka_unit_test(upload_flipped),
		cmocka_unit_test(upload_flipped_height_zero),
		cmocka_unit_test(pow2),
		cmocka_unit_test(mip_rules),
		cmocka_unit_test(mip_rule_rows),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
