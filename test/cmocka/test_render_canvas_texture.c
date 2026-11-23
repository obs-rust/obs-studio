/* Characterization of the canvas texture draw entry points (#5530):
 * obs_render_main_texture(), obs_render_main_texture_src_color_only(),
 * obs_render_canvas_texture() and obs_render_canvas_texture_src_color_only(),
 * and the obs_render_canvas_texture_internal() they share, driven through a
 * real graphics device.
 *
 * test_canvas_draw_steps pins the decisions the draw takes (skip, technique,
 * white level, multiplier) in isolation; this drives the four callers on a
 * live core. The main canvas shows a box whose left half is (r, g, b, 0) and
 * right half (r, g, b, 1), written as they are (blend ONE, ZERO): red on the
 * main canvas, blue on an auxiliary canvas with its own mix. Each case clears
 * a render target to green at half alpha, (0, 1, 0, 0.5), calls one entry
 * point and reads the target back. Colours are 0 or 1 so the sRGB transfer
 * leaves them alone; the destination alpha shows which alpha factors were
 * used. What it pins:
 *
 *   - a canvas with no mix draws nothing and leaves the target unchanged;
 *   - a rendered mix draws its render texture over the target: the plain
 *     variants blend colour with ONE, INVSRCALPHA (where the box is
 *     transparent the green shows through), the _src_color_only variants
 *     with ONE, ZERO (the destination colour is dropped), and all four blend
 *     alpha with ONE, INVSRCALPHA (0.5 stays where the box is transparent,
 *     1 where it is opaque);
 *   - the main-texture variants draw the main canvas's mix, never the
 *     auxiliary canvas's, and the canvas variants draw the canvas they are
 *     given (the main canvas or the auxiliary one);
 *   - the blend state and the framebuffer sRGB flag the caller had are
 *     restored: each draw runs with the caller's blend set to ZERO, ONE (keep
 *     the destination), and a white quad drawn after it must not show; each
 *     draw runs with the sRGB flag off and again on, and the flag is the same
 *     after it.
 *
 * Not reachable through the public API, so left to the Rust swap's parity: a
 * mix whose texture_rendered flag is false (the video thread sets it on the
 * first frame and clears it only when the mix goes inactive or is freed), and
 * a NULL main canvas.
 *
 * Core video needs a graphics module and libobs needs an X display on Linux,
 * so the cases skip without one (run under xvfb-run). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <graphics/vec4.h>
#include <util/platform.h>
#include <util/threading.h>

#define BOX_ID "test_render_canvas_texture_box"
#define SIZE 8
/* sample away from the edge between the halves */
#define LEFT_X 1
#define RIGHT_X (SIZE - 2)
#define ROW (SIZE / 2)
#define TOLERANCE 2

static bool obs_up;
static obs_source_t *main_box;
static obs_source_t *aux_box;
static obs_canvas_t *aux_canvas;
static obs_canvas_t *bare_canvas;

/* ------------------------------------------------------------------------- */
/* a box of one colour, transparent in its left half and opaque in its right */

struct box {
	float r, g, b;
	volatile long renders;
};

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static void *box_create(obs_data_t *settings, obs_source_t *source)
{
	(void)source;
	struct box *b = bzalloc(sizeof(*b));
	b->r = (float)obs_data_get_double(settings, "r");
	b->g = (float)obs_data_get_double(settings, "g");
	b->b = (float)obs_data_get_double(settings, "b");
	return b;
}

static void box_destroy(void *data)
{
	bfree(data);
}

static uint32_t box_size(void *data)
{
	(void)data;
	return SIZE;
}

static void box_half(struct box *b, float x, float alpha)
{
	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	struct vec4 color;
	vec4_set(&color, b->r, b->g, b->b, alpha);
	gs_effect_set_vec4(gs_effect_get_param_by_name(solid, "color"), &color);

	gs_matrix_push();
	gs_matrix_translate3f(x, 0.0f, 0.0f);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, SIZE / 2, SIZE);
	gs_matrix_pop();
}

static void box_render(void *data, gs_effect_t *effect)
{
	struct box *b = data;
	(void)effect;

	gs_blend_state_push();
	gs_blend_function(GS_BLEND_ONE, GS_BLEND_ZERO);
	box_half(b, 0.0f, 0.0f);
	box_half(b, (float)(SIZE / 2), 1.0f);
	gs_blend_state_pop();

	os_atomic_inc_long(&b->renders);
}

static void register_box(void)
{
	struct obs_source_info box = {
		.id = BOX_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
		.get_name = box_name,
		.create = box_create,
		.destroy = box_destroy,
		.get_width = box_size,
		.get_height = box_size,
		.video_render = box_render,
	};
	obs_register_source(&box);
}

static obs_source_t *box(const char *name, double r, double g, double b)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_double(settings, "r", r);
	obs_data_set_double(settings, "g", g);
	obs_data_set_double(settings, "b", b);
	obs_source_t *source = obs_source_create(BOX_ID, name, settings, NULL);
	obs_data_release(settings);
	return source;
}

static long box_renders(obs_source_t *source)
{
	struct box *b = obs_obj_get_data(source);
	return os_atomic_load_long(&b->renders);
}

/* ------------------------------------------------------------------------- */
/* one draw into a cleared target, read back */

enum entry {
	MAIN_TEXTURE,
	MAIN_TEXTURE_SRC_COLOR_ONLY,
	CANVAS_TEXTURE,
	CANVAS_TEXTURE_SRC_COLOR_ONLY,
};

static const char *entry_name(enum entry entry)
{
	switch (entry) {
	case MAIN_TEXTURE:
		return "obs_render_main_texture";
	case MAIN_TEXTURE_SRC_COLOR_ONLY:
		return "obs_render_main_texture_src_color_only";
	case CANVAS_TEXTURE:
		return "obs_render_canvas_texture";
	default:
		return "obs_render_canvas_texture_src_color_only";
	}
}

struct readback {
	uint8_t left[4];
	uint8_t right[4];
	bool srgb_after;
};

static void call_entry(enum entry entry, obs_canvas_t *canvas)
{
	switch (entry) {
	case MAIN_TEXTURE:
		obs_render_main_texture();
		break;
	case MAIN_TEXTURE_SRC_COLOR_ONLY:
		obs_render_main_texture_src_color_only();
		break;
	case CANVAS_TEXTURE:
		obs_render_canvas_texture(canvas);
		break;
	case CANVAS_TEXTURE_SRC_COLOR_ONLY:
		obs_render_canvas_texture_src_color_only(canvas);
		break;
	}
}

/* Clears a target to (0, 1, 0, 0.5), sets the caller's blend to ZERO, ONE
 * and the framebuffer sRGB flag to srgb_before, calls the entry point, notes
 * the sRGB flag, then draws an opaque white quad (which the caller's blend
 * must keep out) and reads the target back. */
static void draw(enum entry entry, obs_canvas_t *canvas, bool srgb_before, struct readback *out)
{
	obs_enter_graphics();
	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	gs_stagesurf_t *stage = gs_stagesurface_create(SIZE, SIZE, GS_RGBA);
	assert_non_null(tr);
	assert_non_null(stage);

	assert_true(gs_texrender_begin(tr, SIZE, SIZE));
	gs_ortho(0.0f, (float)SIZE, 0.0f, (float)SIZE, -100.0f, 100.0f);
	gs_matrix_push();
	gs_matrix_identity();

	const bool srgb_outer = gs_framebuffer_srgb_enabled();
	gs_enable_framebuffer_srgb(srgb_before);

	struct vec4 clear;
	vec4_set(&clear, 0.0f, 1.0f, 0.0f, 0.5f);
	gs_clear(GS_CLEAR_COLOR, &clear, 0.0f, 0);

	gs_blend_state_push();
	gs_enable_blending(true);
	gs_blend_function_separate(GS_BLEND_ZERO, GS_BLEND_ONE, GS_BLEND_ZERO, GS_BLEND_ONE);

	call_entry(entry, canvas);
	out->srgb_after = gs_framebuffer_srgb_enabled();

	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	struct vec4 white;
	vec4_set(&white, 1.0f, 1.0f, 1.0f, 1.0f);
	gs_effect_set_vec4(gs_effect_get_param_by_name(solid, "color"), &white);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, SIZE, SIZE);

	gs_blend_state_pop();
	gs_enable_framebuffer_srgb(srgb_outer);
	gs_matrix_pop();
	gs_texrender_end(tr);

	gs_stage_texture(stage, gs_texrender_get_texture(tr));
	uint8_t *mapped;
	uint32_t linesize;
	assert_true(gs_stagesurface_map(stage, &mapped, &linesize));
	memcpy(out->left, mapped + (size_t)ROW * linesize + LEFT_X * 4, 4);
	memcpy(out->right, mapped + (size_t)ROW * linesize + RIGHT_X * 4, 4);
	gs_stagesurface_unmap(stage);

	gs_stagesurface_destroy(stage);
	gs_texrender_destroy(tr);
	obs_leave_graphics();
}

static bool near(const uint8_t got[4], const uint8_t want[4])
{
	for (int i = 0; i < 4; i++) {
		int d = (int)got[i] - (int)want[i];
		if (d < -TOLERANCE || d > TOLERANCE)
			return false;
	}
	return true;
}

/* The target as cleared: green at half alpha. */
static const uint8_t UNCHANGED[4] = {0, 255, 0, 128};

/* What a draw of a box of colour rgb leaves: the plain variants keep the
 * destination's green where the box is transparent, the _src_color_only
 * variants drop it; alpha is 0.5 there in both, 1 where the box is opaque. */
static void expected(const uint8_t rgb[3], bool src_color_only, uint8_t left[4], uint8_t right[4])
{
	left[0] = rgb[0];
	left[1] = src_color_only ? rgb[1] : 255;
	left[2] = rgb[2];
	left[3] = 128;
	right[0] = rgb[0];
	right[1] = rgb[1];
	right[2] = rgb[2];
	right[3] = 255;
}

static const uint8_t RED[3] = {255, 0, 0};
static const uint8_t BLUE[3] = {0, 0, 255};

/* Draws with the sRGB flag off and on, and checks both draws against
 * left/right and that the flag came back as it was. */
static void check(enum entry entry, obs_canvas_t *canvas, const uint8_t left[4], const uint8_t right[4])
{
	int bad = 0;
	for (int s = 0; s < 2; s++) {
		const bool srgb = s == 1;
		struct readback rb;
		draw(entry, canvas, srgb, &rb);
		if (!near(rb.left, left) || !near(rb.right, right)) {
			fprintf(stderr,
				"%s (sRGB %s): left RGBA %u %u %u %u, right %u %u %u %u; want left %u %u %u %u, right %u %u %u %u\n",
				entry_name(entry), srgb ? "on" : "off", rb.left[0], rb.left[1], rb.left[2], rb.left[3],
				rb.right[0], rb.right[1], rb.right[2], rb.right[3], left[0], left[1], left[2], left[3],
				right[0], right[1], right[2], right[3]);
			bad++;
		}
		if (rb.srgb_after != srgb) {
			fprintf(stderr, "%s: framebuffer sRGB %s before the draw, %s after\n", entry_name(entry),
				srgb ? "on" : "off", rb.srgb_after ? "on" : "off");
			bad++;
		}
	}
	assert_int_equal(bad, 0);
}

static void check_box(enum entry entry, obs_canvas_t *canvas, const uint8_t rgb[3])
{
	const bool src_color_only = entry == MAIN_TEXTURE_SRC_COLOR_ONLY || entry == CANVAS_TEXTURE_SRC_COLOR_ONLY;
	uint8_t left[4], right[4];
	expected(rgb, src_color_only, left, right);
	check(entry, canvas, left, right);
}

/* ------------------------------------------------------------------------- */

/* obs_render_main_texture draws the main canvas's red box over the green:
 * yellow where it is transparent, red where it is opaque. */
static void main_texture_blends_over_destination(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_box(MAIN_TEXTURE, NULL, RED);
}

/* The _src_color_only variant drops the destination colour: red in both
 * halves, the destination alpha kept where the box is transparent. */
static void main_texture_src_color_only_drops_destination_color(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_box(MAIN_TEXTURE_SRC_COLOR_ONLY, NULL, RED);
}

/* Given the main canvas, the canvas variants draw what the main-texture
 * variants draw. */
static void canvas_texture_draws_main_canvas(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	obs_canvas_t *main_canvas = obs_get_main_canvas();
	assert_non_null(main_canvas);
	check_box(CANVAS_TEXTURE, main_canvas, RED);
	check_box(CANVAS_TEXTURE_SRC_COLOR_ONLY, main_canvas, RED);
	obs_canvas_release(main_canvas);
}

/* Given the auxiliary canvas, they draw its blue box, not the main one. */
static void canvas_texture_draws_given_canvas(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_box(CANVAS_TEXTURE, aux_canvas, BLUE);
	check_box(CANVAS_TEXTURE_SRC_COLOR_ONLY, aux_canvas, BLUE);
}

/* A canvas with no mix draws nothing: the target stays green, and the
 * caller's blend and sRGB flag are as they were. */
static void canvas_without_mix_draws_nothing(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check(CANVAS_TEXTURE, bare_canvas, UNCHANGED, UNCHANGED);
	check(CANVAS_TEXTURE_SRC_COLOR_ONLY, bare_canvas, UNCHANGED, UNCHANGED);
}

/* With an auxiliary canvas rendering too, the main-texture variants still
 * draw the main canvas, before and after the auxiliary canvas renders. */
static void main_texture_ignores_auxiliary_canvas(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const long aux_before = box_renders(aux_box);
	check_box(MAIN_TEXTURE, NULL, RED);
	check_box(MAIN_TEXTURE_SRC_COLOR_ONLY, NULL, RED);
	for (int i = 0; i < 300 && box_renders(aux_box) < aux_before + 2; i++)
		os_sleep_ms(10);
	assert_true(box_renders(aux_box) >= aux_before + 2);
	check_box(MAIN_TEXTURE, NULL, RED);
	check_box(MAIN_TEXTURE_SRC_COLOR_ONLY, NULL, RED);
}

/* ------------------------------------------------------------------------- */

/* Waits until a draw of canvas through entry reads back the expected box,
 * for up to 3 s: the first frames of a new mix, and the device right after
 * obs_reset_video() (see test_sprite_flip_render), read back empty. */
static bool settle(enum entry entry, obs_canvas_t *canvas, obs_source_t *source, const uint8_t rgb[3])
{
	uint8_t left[4], right[4];
	expected(rgb, false, left, right);
	int settled = 0;
	for (int i = 0; i < 300 && settled < 3; i++) {
		struct readback rb;
		if (box_renders(source) > 0) {
			draw(entry, canvas, false, &rb);
			if (near(rb.left, left) && near(rb.right, right)) {
				settled++;
				continue;
			}
		}
		settled = 0;
		os_sleep_ms(10);
	}
	return settled >= 3;
}

static void release_all(void)
{
	obs_set_output_source(0, NULL);
	if (aux_canvas) {
		obs_canvas_set_channel(aux_canvas, 0, NULL);
		obs_canvas_remove(aux_canvas);
		obs_canvas_release(aux_canvas);
		aux_canvas = NULL;
	}
	if (bare_canvas) {
		obs_canvas_remove(bare_canvas);
		obs_canvas_release(bare_canvas);
		bare_canvas = NULL;
	}
	obs_source_release(main_box);
	obs_source_release(aux_box);
	main_box = NULL;
	aux_box = NULL;
}

static int group_setup(void **state)
{
	(void)state;
	const char *display = getenv("DISPLAY");
	if (!display || !*display) {
		fprintf(stderr, "skipped: no display\n");
		return 0;
	}
	if (!obs_startup("en-US", NULL, NULL)) {
		fprintf(stderr, "skipped: obs_startup failed\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = SIZE,
		.base_height = SIZE,
		.output_width = SIZE,
		.output_height = SIZE,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_POINT,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		return 0;
	}

	register_box();
	main_box = box("main box", 1.0, 0.0, 0.0);
	aux_box = box("aux box", 0.0, 0.0, 1.0);
	assert_non_null(main_box);
	assert_non_null(aux_box);
	obs_set_output_source(0, main_box);

	aux_canvas = obs_canvas_create("aux", &ovi, DEVICE);
	assert_non_null(aux_canvas);
	obs_canvas_set_channel(aux_canvas, 0, aux_box);

	/* NULL video info: a canvas without a mix */
	bare_canvas = obs_canvas_create("bare", NULL, DEVICE);
	assert_non_null(bare_canvas);

	if (!settle(MAIN_TEXTURE, NULL, main_box, RED) || !settle(CANVAS_TEXTURE, aux_canvas, aux_box, BLUE)) {
		fprintf(stderr, "a mix never read back its box (main renders %ld, aux renders %ld)\n",
			box_renders(main_box), box_renders(aux_box));
		release_all();
		obs_shutdown();
		return -1;
	}

	obs_up = true;
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up) {
		release_all();
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(main_texture_blends_over_destination),
		cmocka_unit_test(main_texture_src_color_only_drops_destination_color),
		cmocka_unit_test(canvas_texture_draws_main_canvas),
		cmocka_unit_test(canvas_texture_draws_given_canvas),
		cmocka_unit_test(canvas_without_mix_draws_nothing),
		cmocka_unit_test(main_texture_ignores_auxiliary_canvas),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
