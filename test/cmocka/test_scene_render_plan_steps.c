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

#include "obs-scene-render-plan.h"

/*
 * Characterization of the per-item render decisions of render_item() and
 * render_item_texture() in libobs/obs-scene.c, moved to
 * libobs/obs-scene-render-plan.c before their Rust port:
 * - whether the item is drawn through its texrender, and what happens to the
 *   texrender;
 * - the empty-source rule (#573) and the clear colour (#1406, white for
 *   Darken);
 * - the child drawn (show transition, hide transition or source) and the
 *   linear sRGB state;
 * - whether a directly drawn source gets centered texture coordinates;
 * - the effect, the point sampler, the base dimension parameters and Area's
 *   upscale, from the scale filter and the output scale;
 * - the SDR white multiplier and the technique from the two colour spaces.
 * Cases marked "characterized, not endorsed" are odd but real.
 *
 * The functions are hidden in libobs, so this test compiles the C source
 * into itself and calls only them. test_item_transition_empty_source (#573),
 * test_darken_transparent (#1406), test_scene_rotated_scale_filter,
 * test_scene_blend, test_visibility_transition and
 * test_group_hide_transition_audio cover the caller through the real libobs.
 */

/* enum obs_scale_type, enum obs_blending_method, enum obs_blending_type and
 * enum gs_color_space (obs.h and graphics.h); obs-scene.c checks the
 * header's values against them */
#define SCALE_DISABLE OBS_SCENE_RENDER_SCALE_DISABLE
#define SCALE_POINT OBS_SCENE_RENDER_SCALE_POINT
#define SCALE_BICUBIC OBS_SCENE_RENDER_SCALE_BICUBIC
#define SCALE_BILINEAR 3
#define SCALE_LANCZOS OBS_SCENE_RENDER_SCALE_LANCZOS
#define SCALE_AREA OBS_SCENE_RENDER_SCALE_AREA
#define METHOD_DEFAULT 0
#define METHOD_SRGB_OFF OBS_SCENE_RENDER_BLEND_METHOD_SRGB_OFF
#define BLEND_NORMAL OBS_SCENE_RENDER_BLEND_NORMAL
#define BLEND_DARKEN OBS_SCENE_RENDER_BLEND_DARKEN
#define CS_SRGB OBS_SCENE_RENDER_CS_SRGB
#define CS_SRGB_16F OBS_SCENE_RENDER_CS_SRGB_16F
#define CS_709_EXTENDED OBS_SCENE_RENDER_CS_709_EXTENDED
#define CS_709_SCRGB OBS_SCENE_RENDER_CS_709_SCRGB

#define EFFECT_DEFAULT OBS_SCENE_RENDER_EFFECT_DEFAULT
#define EFFECT_LOWRES OBS_SCENE_RENDER_EFFECT_BILINEAR_LOWRES
#define EFFECT_BICUBIC OBS_SCENE_RENDER_EFFECT_BICUBIC
#define EFFECT_LANCZOS OBS_SCENE_RENDER_EFFECT_LANCZOS
#define EFFECT_AREA OBS_SCENE_RENDER_EFFECT_AREA

static uint32_t bits_of(float f)
{
	uint32_t u;
	memcpy(&u, &f, sizeof(u));
	return u;
}

static float float_of(uint32_t u)
{
	float f;
	memcpy(&f, &u, sizeof(f));
	return f;
}

/* ------------------------------------------------------------------------ */
/* Texture triggers */

static struct obs_scene_render_triggers no_triggers(void)
{
	struct obs_scene_render_triggers t;
	memset(&t, 0, sizeof(t));
	t.scale_filter = SCALE_DISABLE;
	t.blend_method = METHOD_DEFAULT;
	t.blend_type = BLEND_NORMAL;
	return t;
}

static void check_triggers_line(const struct obs_scene_render_triggers *t, bool expected, const char *what, int line)
{
	const bool actual = obs_scene_render_texture_enabled(t);
	if (actual != expected) {
		fprintf(stderr, "line %d: %s: texture %s, expected %s\n", line, what, actual ? "enabled" : "disabled",
			expected ? "enabled" : "disabled");
		fail();
	}
}

#define check_triggers(t, expected, what) check_triggers_line(t, expected, what, __LINE__)

static void texture_no_trigger(void **state)
{
	(void)state;

	struct obs_scene_render_triggers t = no_triggers();
	check_triggers(&t, false, "nothing set");

	/* a group is a scene, but drawn directly */
	t.is_group = true;
	check_triggers(&t, false, "a group flag alone");
	t.is_scene = true;
	check_triggers(&t, false, "a group (a scene and a group)");
}

static void texture_each_trigger_alone(void **state)
{
	(void)state;

	static const int crop_values[] = {1, -1, INT_MAX, INT_MIN};

	for (size_t side = 0; side < 8; side++) {
		for (size_t v = 0; v < sizeof(crop_values) / sizeof(crop_values[0]); v++) {
			struct obs_scene_render_triggers t = no_triggers();
			int *fields[] = {&t.crop_left,         &t.crop_top,          &t.crop_right,
					 &t.crop_bottom,       &t.bounds_crop_left,  &t.bounds_crop_top,
					 &t.bounds_crop_right, &t.bounds_crop_bottom};
			char what[64];
			*fields[side] = crop_values[v];
			snprintf(what, sizeof(what), "crop field %zu = %d", side, crop_values[v]);
			check_triggers(&t, true, what);
		}
	}

	/* every scale filter but Disable, values outside the enum included */
	static const int filters[] = {SCALE_POINT, SCALE_BICUBIC, SCALE_BILINEAR, SCALE_LANCZOS, SCALE_AREA, 6,
				      -1,          INT_MAX,       INT_MIN};
	for (size_t i = 0; i < sizeof(filters) / sizeof(filters[0]); i++) {
		struct obs_scene_render_triggers t = no_triggers();
		char what[64];
		t.scale_filter = filters[i];
		snprintf(what, sizeof(what), "scale filter %d", filters[i]);
		check_triggers(&t, true, what);
	}

	/* sRGB off only: another value outside the enum is not a trigger */
	static const struct {
		int method;
		bool expected;
	} methods[] = {{METHOD_SRGB_OFF, true}, {2, false}, {-1, false}, {INT_MAX, false}};
	for (size_t i = 0; i < sizeof(methods) / sizeof(methods[0]); i++) {
		struct obs_scene_render_triggers t = no_triggers();
		char what[64];
		t.blend_method = methods[i].method;
		snprintf(what, sizeof(what), "blend method %d", methods[i].method);
		check_triggers(&t, methods[i].expected, what);
	}

	/* any blend mode but Normal, values outside the enum included */
	static const int blends[] = {1, 2, 3, 4, 5, BLEND_DARKEN, 7, -1, INT_MIN};
	for (size_t i = 0; i < sizeof(blends) / sizeof(blends[0]); i++) {
		struct obs_scene_render_triggers t = no_triggers();
		char what[64];
		t.blend_type = blends[i];
		snprintf(what, sizeof(what), "blend type %d", blends[i]);
		check_triggers(&t, true, what);
	}

	struct obs_scene_render_triggers t = no_triggers();
	t.is_scene = true;
	check_triggers(&t, true, "a nested scene");
}

/* ------------------------------------------------------------------------ */
/* The texrender */

static void texrender_actions(void **state)
{
	(void)state;

	/* has, use, format matches: render_item() passes format_matches true
	 * only with both, so the rows where it is true without them are
	 * characterized, not endorsed (they cannot happen in libobs) */
	static const struct {
		bool has, use, match;
		enum obs_scene_render_texrender expected;
	} rows[] = {
		{false, false, false, OBS_SCENE_RENDER_TEXRENDER_NONE},
		{false, false, true, OBS_SCENE_RENDER_TEXRENDER_NONE},
		{false, true, false, OBS_SCENE_RENDER_TEXRENDER_CREATE},
		{false, true, true, OBS_SCENE_RENDER_TEXRENDER_CREATE},
		{true, false, false, OBS_SCENE_RENDER_TEXRENDER_DESTROY},
		{true, false, true, OBS_SCENE_RENDER_TEXRENDER_DESTROY},
		{true, true, false, OBS_SCENE_RENDER_TEXRENDER_RECREATE},
		{true, true, true, OBS_SCENE_RENDER_TEXRENDER_KEEP},
	};

	for (size_t i = 0; i < sizeof(rows) / sizeof(rows[0]); i++) {
		const enum obs_scene_render_texrender actual =
			obs_scene_render_texrender_action(rows[i].has, rows[i].use, rows[i].match);
		if (actual != rows[i].expected) {
			fprintf(stderr, "has %d use %d match %d: action %d, expected %d\n", rows[i].has, rows[i].use,
				rows[i].match, (int)actual, (int)rows[i].expected);
			fail();
		}
	}
}

/* #573: the empty-source rule's size check */
static void source_size_empty(void **state)
{
	(void)state;

	assert_true(obs_scene_render_size_empty(0, 0));
	assert_true(obs_scene_render_size_empty(0, 1));
	assert_true(obs_scene_render_size_empty(1, 0));
	assert_true(obs_scene_render_size_empty(UINT32_MAX, 0));
	assert_true(obs_scene_render_size_empty(0, UINT32_MAX));
	assert_false(obs_scene_render_size_empty(1, 1));
	assert_false(obs_scene_render_size_empty(1920, 1080));
	assert_false(obs_scene_render_size_empty(UINT32_MAX, UINT32_MAX));
}

/* #1406: white for Darken only */
static void clear_colour(void **state)
{
	(void)state;

	assert_true(obs_scene_render_clear_white(BLEND_DARKEN));
	static const int others[] = {BLEND_NORMAL, 1, 2, 3, 4, 5, 7, -1, INT_MAX, INT_MIN};
	for (size_t i = 0; i < sizeof(others) / sizeof(others[0]); i++) {
		if (obs_scene_render_clear_white(others[i])) {
			fprintf(stderr, "blend type %d clears to white\n", others[i]);
			fail();
		}
	}
}

/* ------------------------------------------------------------------------ */
/* The child */

static void child_choice(void **state)
{
	(void)state;

	/* visible, show active, hide active. render_item() asks for the show
	 * transition only when visible and the hide transition only when
	 * hidden, so it passes false for the other; with it true anyway the
	 * other is ignored */
	static const struct {
		bool visible, show, hide;
		enum obs_scene_render_child_id expected;
	} rows[] = {
		{true, false, false, OBS_SCENE_RENDER_CHILD_SOURCE},
		{true, true, false, OBS_SCENE_RENDER_CHILD_SHOW},
		{true, false, true, OBS_SCENE_RENDER_CHILD_SOURCE},
		{true, true, true, OBS_SCENE_RENDER_CHILD_SHOW},
		{false, false, false, OBS_SCENE_RENDER_CHILD_SOURCE},
		{false, true, false, OBS_SCENE_RENDER_CHILD_SOURCE},
		{false, false, true, OBS_SCENE_RENDER_CHILD_HIDE},
		{false, true, true, OBS_SCENE_RENDER_CHILD_HIDE},
	};

	for (size_t i = 0; i < sizeof(rows) / sizeof(rows[0]); i++) {
		const enum obs_scene_render_child_id actual =
			obs_scene_render_child(rows[i].visible, rows[i].show, rows[i].hide);
		if (actual != rows[i].expected) {
			fprintf(stderr, "visible %d show %d hide %d: child %d, expected %d\n", rows[i].visible,
				rows[i].show, rows[i].hide, (int)actual, (int)rows[i].expected);
			fail();
		}
	}
}

static void linear_srgb(void **state)
{
	(void)state;

	/* drawn directly: always linear */
	assert_true(obs_scene_render_linear_srgb(false, METHOD_DEFAULT));
	assert_true(obs_scene_render_linear_srgb(false, METHOD_SRGB_OFF));
	assert_true(obs_scene_render_linear_srgb(false, -1));
	/* through the texrender: unless sRGB is off */
	assert_true(obs_scene_render_linear_srgb(true, METHOD_DEFAULT));
	assert_false(obs_scene_render_linear_srgb(true, METHOD_SRGB_OFF));
	assert_true(obs_scene_render_linear_srgb(true, 2));
	assert_true(obs_scene_render_linear_srgb(true, -1));
}

/* ------------------------------------------------------------------------ */
/* Centered texture coordinates */

static struct matrix4 identity_with(float tx, float ty)
{
	struct matrix4 m;
	memset(&m, 0, sizeof(m));
	m.x.x = 1.0f;
	m.y.y = 1.0f;
	m.z.z = 1.0f;
	m.t.w = 1.0f;
	m.t.x = tx;
	m.t.y = ty;
	return m;
}

static void texcoords_identity(void **state)
{
	(void)state;

	struct matrix4 m = identity_with(0.0f, 0.0f);
	assert_true(obs_scene_texcoords_centered(&m));

	m = identity_with(3.0f, -7.0f);
	assert_true(obs_scene_texcoords_centered(&m));

	m = identity_with(16777216.0f, -16777216.0f);
	assert_true(obs_scene_texcoords_centered(&m));

	/* a scale, a rotation or any other entry moved: not centred */
	m = identity_with(0.0f, 0.0f);
	m.x.x = 2.0f;
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(0.0f, 0.0f);
	m.x.y = 1.0f;
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(0.0f, 0.0f);
	m.t.z = 1.0f;
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(0.0f, 0.0f);
	m.t.w = 2.0f;
	assert_false(obs_scene_texcoords_centered(&m));
}

static void texcoords_fractional_translation(void **state)
{
	(void)state;

	struct matrix4 m = identity_with(0.5f, 0.0f);
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(0.0f, 0.5f);
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(-0.5f, 0.0f);
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(100.25f, 7.0f);
	assert_false(obs_scene_texcoords_centered(&m));
	/* the float just above a whole number */
	m = identity_with(nextafterf(3.0f, 4.0f), 0.0f);
	assert_false(obs_scene_texcoords_centered(&m));
	/* the smallest subnormal */
	m = identity_with(float_of(1), 0.0f);
	assert_false(obs_scene_texcoords_centered(&m));
}

static void texcoords_negative_zero(void **state)
{
	(void)state;

	/* floorf(-0.0) is -0.0, so a -0.0 translation compares equal */
	struct matrix4 m = identity_with(-0.0f, -0.0f);
	assert_true(obs_scene_texcoords_centered(&m));

	/* characterized, not endorsed: the compare is memcmp, so -0.0 anywhere
	 * else is not the identity's 0.0 */
	m = identity_with(0.0f, 0.0f);
	m.x.y = -0.0f;
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(0.0f, 0.0f);
	m.y.x = -0.0f;
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(0.0f, 0.0f);
	m.t.z = -0.0f;
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(0.0f, 0.0f);
	m.x.w = -0.0f;
	assert_false(obs_scene_texcoords_centered(&m));
}

static void texcoords_nan(void **state)
{
	(void)state;

	/* characterized, not endorsed: floorf() gives a quiet NaN back as it
	 * is, so a quiet NaN translation compares equal, and an infinite one
	 * too */
	struct matrix4 m = identity_with(float_of(0x7fc00000u), 0.0f);
	assert_true(obs_scene_texcoords_centered(&m));
	m = identity_with(0.0f, float_of(0xffc00001u));
	assert_true(obs_scene_texcoords_centered(&m));
	m = identity_with(INFINITY, -INFINITY);
	assert_true(obs_scene_texcoords_centered(&m));

	/* a NaN anywhere else is not the identity */
	m = identity_with(0.0f, 0.0f);
	m.x.x = NAN;
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(0.0f, 0.0f);
	m.t.z = NAN;
	assert_false(obs_scene_texcoords_centered(&m));
	m = identity_with(0.0f, 0.0f);
	m.t.w = NAN;
	assert_false(obs_scene_texcoords_centered(&m));
}

/* ------------------------------------------------------------------------ */
/* The effect */

static void check_effect_line(int filter, float sx, float sy, int effect, bool point_sampler, bool base_dimension,
			      bool upscale, int line)
{
	struct obs_scene_render_effect_plan plan;
	memset(&plan, 0xa5, sizeof(plan));
	obs_scene_render_effect(filter, sx, sy, &plan);
	if (plan.effect != effect || plan.point_sampler != point_sampler || plan.base_dimension != base_dimension ||
	    plan.upscale != upscale) {
		fprintf(stderr,
			"line %d: filter %d scale (%a, %a): effect %d point %d base %d upscale %d, expected %d %d %d %d\n",
			line, filter, (double)sx, (double)sy, plan.effect, plan.point_sampler, plan.base_dimension,
			plan.upscale, effect, point_sampler, base_dimension, upscale);
		fail();
	}
}

#define check_effect(filter, sx, sy, effect, point, base, upscale) \
	check_effect_line(filter, sx, sy, effect, point, base, upscale, __LINE__)

static void effect_each_filter(void **state)
{
	(void)state;

	/* Disable: nothing at any scale */
	check_effect(SCALE_DISABLE, 1.0f, 1.0f, EFFECT_DEFAULT, false, false, false);
	check_effect(SCALE_DISABLE, 2.0f, 2.0f, EFFECT_DEFAULT, false, false, false);
	check_effect(SCALE_DISABLE, 0.25f, 0.25f, EFFECT_DEFAULT, false, false, false);
	check_effect(SCALE_DISABLE, NAN, NAN, EFFECT_DEFAULT, false, false, false);

	/* Point: the point sampler on the default effect, at any scale, and no
	 * base dimension */
	check_effect(SCALE_POINT, 1.0f, 1.0f, EFFECT_DEFAULT, true, false, false);
	check_effect(SCALE_POINT, 2.0f, 2.0f, EFFECT_DEFAULT, true, false, false);
	check_effect(SCALE_POINT, 0.25f, 0.25f, EFFECT_DEFAULT, true, false, false);
	check_effect(SCALE_POINT, NAN, 1.0f, EFFECT_DEFAULT, true, false, false);

	/* the others at scale 1: nothing */
	check_effect(SCALE_BICUBIC, 1.0f, 1.0f, EFFECT_DEFAULT, false, false, false);
	check_effect(SCALE_BILINEAR, 1.0f, 1.0f, EFFECT_DEFAULT, false, false, false);
	check_effect(SCALE_LANCZOS, 1.0f, 1.0f, EFFECT_DEFAULT, false, false, false);
	check_effect(SCALE_AREA, 1.0f, 1.0f, EFFECT_DEFAULT, false, false, false);

	/* scaled up on both axes: Area upscales */
	check_effect(SCALE_BICUBIC, 2.0f, 2.0f, EFFECT_BICUBIC, false, true, false);
	check_effect(SCALE_BILINEAR, 2.0f, 2.0f, EFFECT_DEFAULT, false, true, false);
	check_effect(SCALE_LANCZOS, 2.0f, 2.0f, EFFECT_LANCZOS, false, true, false);
	check_effect(SCALE_AREA, 2.0f, 2.0f, EFFECT_AREA, false, true, true);

	/* scaled down a little: Area does not upscale */
	check_effect(SCALE_BICUBIC, 0.75f, 0.75f, EFFECT_BICUBIC, false, true, false);
	check_effect(SCALE_BILINEAR, 0.75f, 0.75f, EFFECT_DEFAULT, false, true, false);
	check_effect(SCALE_LANCZOS, 0.75f, 0.75f, EFFECT_LANCZOS, false, true, false);
	check_effect(SCALE_AREA, 0.75f, 0.75f, EFFECT_AREA, false, true, false);

	/* one axis 1, the other up: Area upscales (1 >= 1) */
	check_effect(SCALE_AREA, 1.0f, 2.0f, EFFECT_AREA, false, true, true);
	check_effect(SCALE_AREA, 2.0f, 1.0f, EFFECT_AREA, false, true, true);
	/* one axis up, the other down: no upscale */
	check_effect(SCALE_AREA, 2.0f, 0.75f, EFFECT_AREA, false, true, false);
	check_effect(SCALE_AREA, 0.75f, 2.0f, EFFECT_AREA, false, true, false);
	check_effect(SCALE_LANCZOS, 2.0f, 0.75f, EFFECT_LANCZOS, false, true, false);

	/* below 0.5 on either axis: the low-resolution bilinear effect, for
	 * every filter but Point, and Area's upscale is not computed */
	check_effect(SCALE_BICUBIC, 0.25f, 0.25f, EFFECT_LOWRES, false, true, false);
	check_effect(SCALE_BILINEAR, 0.25f, 0.25f, EFFECT_LOWRES, false, true, false);
	check_effect(SCALE_LANCZOS, 0.25f, 0.25f, EFFECT_LOWRES, false, true, false);
	check_effect(SCALE_AREA, 0.25f, 0.25f, EFFECT_LOWRES, false, true, false);
	check_effect(SCALE_AREA, 0.25f, 4.0f, EFFECT_LOWRES, false, true, false);
	check_effect(SCALE_AREA, 4.0f, 0.25f, EFFECT_LOWRES, false, true, false);

	/* characterized, not endorsed: a negative scale is below 0.5 */
	check_effect(SCALE_BICUBIC, -2.0f, 2.0f, EFFECT_LOWRES, false, true, false);

	/* a filter outside the enum is not Disable or Point: the default
	 * effect, with the base dimension, when scaled */
	check_effect(6, 2.0f, 2.0f, EFFECT_DEFAULT, false, true, false);
	check_effect(-1, 0.75f, 0.75f, EFFECT_DEFAULT, false, true, false);
	check_effect(-1, 0.25f, 0.25f, EFFECT_LOWRES, false, true, false);
	check_effect(INT_MAX, 1.0f, 1.0f, EFFECT_DEFAULT, false, false, false);
}

static void effect_output_scale_edges(void **state)
{
	(void)state;

	/* 0.5 and the floats on either side: below 0.5 is low resolution */
	const float below_half = nextafterf(0.5f, 0.0f);
	const float above_half = nextafterf(0.5f, 1.0f);
	check_effect(SCALE_BICUBIC, below_half, 1.0f, EFFECT_LOWRES, false, true, false);
	check_effect(SCALE_BICUBIC, 0.5f, 1.0f, EFFECT_BICUBIC, false, true, false);
	check_effect(SCALE_BICUBIC, above_half, 1.0f, EFFECT_BICUBIC, false, true, false);
	check_effect(SCALE_LANCZOS, 1.0f, below_half, EFFECT_LOWRES, false, true, false);
	check_effect(SCALE_LANCZOS, 1.0f, 0.5f, EFFECT_LANCZOS, false, true, false);
	check_effect(SCALE_AREA, 0.5f, 0.5f, EFFECT_AREA, false, true, false);
	check_effect(SCALE_AREA, below_half, 0.5f, EFFECT_LOWRES, false, true, false);

	/* 1 +- EPSILON (1e-4f): fabsf(scale - 1) <= EPSILON counts as 1. 1 +
	 * EPSILON and 1 - EPSILON round away from 1 as floats, so they are not
	 * close; the next floats towards 1 are */
	const float one_plus_eps = 1.0f + 1e-4f;
	const float one_minus_eps = 1.0f - 1e-4f;
	assert_int_equal(bits_of(one_plus_eps), 0x3f800347u);
	assert_int_equal(bits_of(one_minus_eps), 0x3f7ff972u);
	const float close_above = float_of(0x3f800346u);
	const float close_below = float_of(0x3f7ff973u);

	check_effect(SCALE_BICUBIC, one_plus_eps, 1.0f, EFFECT_BICUBIC, false, true, false);
	check_effect(SCALE_BICUBIC, close_above, 1.0f, EFFECT_DEFAULT, false, false, false);
	check_effect(SCALE_BICUBIC, 1.0f, one_minus_eps, EFFECT_BICUBIC, false, true, false);
	check_effect(SCALE_BICUBIC, 1.0f, close_below, EFFECT_DEFAULT, false, false, false);
	check_effect(SCALE_BICUBIC, close_below, close_above, EFFECT_DEFAULT, false, false, false);

	/* Area: 1 + EPSILON on one axis enters the branch and upscales (both
	 * >= 1); 1 - EPSILON does not */
	check_effect(SCALE_AREA, one_plus_eps, 1.0f, EFFECT_AREA, false, true, true);
	check_effect(SCALE_AREA, one_plus_eps, close_below, EFFECT_AREA, false, true, false);
	check_effect(SCALE_AREA, one_minus_eps, 1.0f, EFFECT_AREA, false, true, false);
	check_effect(SCALE_AREA, close_above, close_below, EFFECT_DEFAULT, false, false, false);
}

static void effect_output_scale_nan(void **state)
{
	(void)state;

	/* characterized, not endorsed: a NaN scale is never close to 1 and
	 * never below 0.5, so the filter's own effect is used, and Area does not
	 * upscale */
	const float nan_q = float_of(0x7fc00000u);
	const float nan_neg = float_of(0xffc12345u);
	check_effect(SCALE_BICUBIC, nan_q, 1.0f, EFFECT_BICUBIC, false, true, false);
	check_effect(SCALE_BICUBIC, 1.0f, nan_neg, EFFECT_BICUBIC, false, true, false);
	check_effect(SCALE_LANCZOS, nan_q, nan_q, EFFECT_LANCZOS, false, true, false);
	check_effect(SCALE_BILINEAR, nan_q, 1.0f, EFFECT_DEFAULT, false, true, false);
	check_effect(SCALE_AREA, nan_q, 2.0f, EFFECT_AREA, false, true, false);
	check_effect(SCALE_AREA, 2.0f, nan_neg, EFFECT_AREA, false, true, false);
	/* the other axis below 0.5 still wins */
	check_effect(SCALE_AREA, nan_q, 0.25f, EFFECT_LOWRES, false, true, false);
	/* infinity is scaled up */
	check_effect(SCALE_AREA, INFINITY, INFINITY, EFFECT_AREA, false, true, true);
	check_effect(SCALE_AREA, -INFINITY, 1.0f, EFFECT_LOWRES, false, true, false);
}

/* ------------------------------------------------------------------------ */
/* The multiplier and the technique */

static const int spaces[] = {CS_SRGB, CS_SRGB_16F, CS_709_EXTENDED, CS_709_SCRGB};

static bool is_sdr_or_extended(int space)
{
	return space == CS_SRGB || space == CS_SRGB_16F || space == CS_709_EXTENDED;
}

static void check_multiplier_line(int current, int source, float white, float expected, int line)
{
	const float actual = obs_scene_render_multiplier(current, source, white);
	if (bits_of(actual) != bits_of(expected)) {
		fprintf(stderr, "line %d: current %d source %d white %a: multiplier %a (%08x), expected %a (%08x)\n",
			line, current, source, (double)white, (double)actual, bits_of(actual), (double)expected,
			bits_of(expected));
		fail();
	}
}

static void multiplier_space_pairs(void **state)
{
	(void)state;

	static const float whites[] = {80.0f, 300.0f, 0.0f};

	for (size_t w = 0; w < sizeof(whites) / sizeof(whites[0]); w++) {
		const float white = whites[w];
		for (size_t c = 0; c < 4; c++) {
			for (size_t s = 0; s < 4; s++) {
				const int current = spaces[c];
				const int source = spaces[s];
				float expected = 1.0f;
				/* SDR or extended drawn into scRGB: white / 80 */
				if (current == CS_709_SCRGB && is_sdr_or_extended(source))
					expected = white / 80.0f;
				/* scRGB drawn into SDR or extended: 80 / white */
				if (source == CS_709_SCRGB && is_sdr_or_extended(current))
					expected = 80.0f / white;
				check_multiplier_line(current, source, white, expected, __LINE__);
			}
		}
	}

	/* the values, written out */
	check_multiplier_line(CS_709_SCRGB, CS_SRGB, 300.0f, 3.75f, __LINE__);
	check_multiplier_line(CS_709_SCRGB, CS_709_EXTENDED, 80.0f, 1.0f, __LINE__);
	check_multiplier_line(CS_SRGB, CS_709_SCRGB, 300.0f, float_of(0x3e888889u), __LINE__);
	check_multiplier_line(CS_SRGB_16F, CS_709_SCRGB, 80.0f, 1.0f, __LINE__);
	check_multiplier_line(CS_709_SCRGB, CS_709_SCRGB, 300.0f, 1.0f, __LINE__);
	check_multiplier_line(CS_SRGB, CS_SRGB, 300.0f, 1.0f, __LINE__);
	check_multiplier_line(CS_709_SCRGB, CS_SRGB_16F, 0.0f, 0.0f, __LINE__);

	/* characterized, not endorsed: a white level of 0 makes 80 / 0, +inf */
	check_multiplier_line(CS_SRGB, CS_709_SCRGB, 0.0f, INFINITY, __LINE__);
	check_multiplier_line(CS_709_EXTENDED, CS_709_SCRGB, 0.0f, INFINITY, __LINE__);
	check_multiplier_line(CS_709_EXTENDED, CS_709_SCRGB, -0.0f, -INFINITY, __LINE__);

	/* a space outside the enum gives 1 */
	check_multiplier_line(CS_709_SCRGB, 4, 300.0f, 1.0f, __LINE__);
	check_multiplier_line(-1, CS_709_SCRGB, 300.0f, 1.0f, __LINE__);
	check_multiplier_line(4, -1, 300.0f, 1.0f, __LINE__);
}

#define T_DRAW OBS_SCENE_RENDER_TECH_DRAW
#define T_MUL OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY
#define T_TONE OBS_SCENE_RENDER_TECH_DRAW_TONEMAP
#define T_MUL_TONE OBS_SCENE_RENDER_TECH_DRAW_MULTIPLY_TONEMAP
#define T_UP OBS_SCENE_RENDER_TECH_DRAW_UPSCALE
#define T_UP_MUL OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY
#define T_UP_TONE OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_TONEMAP
#define T_UP_MUL_TONE OBS_SCENE_RENDER_TECH_DRAW_UPSCALE_MULTIPLY_TONEMAP

static void technique_space_pairs(void **state)
{
	(void)state;

	/* [source][current], in the order of spaces[] */
	static const enum obs_scene_render_technique_id plain[4][4] = {
		/* sRGB */ {T_DRAW, T_DRAW, T_DRAW, T_MUL},
		/* sRGB 16F */ {T_DRAW, T_DRAW, T_DRAW, T_MUL},
		/* 709 extended */ {T_TONE, T_TONE, T_DRAW, T_MUL},
		/* 709 scRGB */ {T_MUL_TONE, T_MUL_TONE, T_MUL, T_DRAW},
	};
	static const enum obs_scene_render_technique_id upscaled[4][4] = {
		/* sRGB */ {T_UP, T_UP, T_UP, T_UP_MUL},
		/* sRGB 16F */ {T_UP, T_UP, T_UP, T_UP_MUL},
		/* 709 extended */ {T_UP_TONE, T_UP_TONE, T_UP, T_UP_MUL},
		/* 709 scRGB */ {T_UP_MUL_TONE, T_UP_MUL_TONE, T_UP_MUL, T_UP},
	};

	for (size_t s = 0; s < 4; s++) {
		for (size_t c = 0; c < 4; c++) {
			const enum obs_scene_render_technique_id p =
				obs_scene_render_technique(false, spaces[c], spaces[s]);
			const enum obs_scene_render_technique_id u =
				obs_scene_render_technique(true, spaces[c], spaces[s]);
			if (p != plain[s][c] || u != upscaled[s][c]) {
				fprintf(stderr, "source %d current %d: %d / %d, expected %d / %d\n", spaces[s],
					spaces[c], (int)p, (int)u, (int)plain[s][c], (int)upscaled[s][c]);
				fail();
			}
		}
	}

	/* a space outside the enum: Draw or DrawUpscale */
	assert_int_equal(obs_scene_render_technique(false, CS_709_SCRGB, 4), T_DRAW);
	assert_int_equal(obs_scene_render_technique(true, CS_709_SCRGB, -1), T_UP);
	assert_int_equal(obs_scene_render_technique(false, 4, CS_709_SCRGB), T_DRAW);
	assert_int_equal(obs_scene_render_technique(true, -1, CS_709_EXTENDED), T_UP);
	assert_int_equal(obs_scene_render_technique(true, INT_MIN, INT_MAX), T_UP);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(texture_no_trigger),
		cmocka_unit_test(texture_each_trigger_alone),
		cmocka_unit_test(texrender_actions),
		cmocka_unit_test(source_size_empty),
		cmocka_unit_test(clear_colour),
		cmocka_unit_test(child_choice),
		cmocka_unit_test(linear_srgb),
		cmocka_unit_test(texcoords_identity),
		cmocka_unit_test(texcoords_fractional_translation),
		cmocka_unit_test(texcoords_negative_zero),
		cmocka_unit_test(texcoords_nan),
		cmocka_unit_test(effect_each_filter),
		cmocka_unit_test(effect_output_scale_edges),
		cmocka_unit_test(effect_output_scale_nan),
		cmocka_unit_test(multiplier_space_pairs),
		cmocka_unit_test(technique_space_pairs),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
