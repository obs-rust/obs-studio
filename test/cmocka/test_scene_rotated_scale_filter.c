/* Regression test for obsproject/obs-studio#12625 (reported by alystair; the
 * same problem was reported on the OBS forum in 2020): a scene item's scale
 * filtering stops working once the item is rotated, so a scaled-down source
 * shows the same aliasing as with no scale filter at all.
 *
 * Below half size every scale filter except Point draws through
 * bilinear_lowres_scale.effect, and Area draws through area.effect from half
 * size up. Both measured the footprint of a screen pixel in the source texture
 * as float2(ddx(uv.x), ddy(uv.y)), which assumes the texture's x axis follows
 * the screen's x axis. Rotated by 90 degrees, uv.x changes along the screen's
 * y axis only, so both derivatives are 0: the low-res filter's eight taps fell
 * on one point (point sampling), and Area summed over nothing (with OpenGL
 * the item came out transparent). At other angles the footprint shrank with the cosine of
 * the angle. Area also drew nothing at 180 degrees, where the derivatives are
 * negative.
 *
 * The test draws 1-pixel black and white stripes through a scaled scene item
 * with a scale filter, reads the canvas back, and compares the middle of the
 * item at several rotations with the unrotated item: a filter that works the
 * same way at any rotation gives the same brightness and the same smoothness
 * (the stripes average to an even grey when scaled down), while point
 * sampling gives runs of black and white. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <math.h>
#include <stdio.h>
#include <stdlib.h>

#include <obs.h>
#include <graphics/vec4.h>
#include <util/platform.h>

#define STRIPES_ID "scene_rotated_scale_filter_stripes"
#define STRIPES_SIZE 256
#define CANVAS_SIZE 256

/* How much brighter or darker (mean) and less even (standard deviation), in
 * 8-bit levels, the middle of the rotated item may be than the unrotated one.
 * A correct filter matches to within a few levels at any angle; point
 * sampling the stripes is off by 25 to 125. */
#define MAX_MEAN_DIFF 6.0
#define MAX_SD_DIFF 6.0

static bool obs_up;

static const char *stripes_name(void *type_data)
{
	(void)type_data;
	return "stripes";
}

struct stripes {
	gs_texture_t *tex;
};

static void *stripes_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	struct stripes *stripes = bzalloc(sizeof(*stripes));
	uint8_t *pixels = bmalloc(STRIPES_SIZE * STRIPES_SIZE * 4);
	for (int y = 0; y < STRIPES_SIZE; y++) {
		for (int x = 0; x < STRIPES_SIZE; x++) {
			uint8_t v = (x & 1) ? 255 : 0;
			uint8_t *p = pixels + ((size_t)y * STRIPES_SIZE + x) * 4;
			p[0] = p[1] = p[2] = v;
			p[3] = 255;
		}
	}
	const uint8_t *planes[] = {pixels};
	obs_enter_graphics();
	stripes->tex = gs_texture_create(STRIPES_SIZE, STRIPES_SIZE, GS_RGBA, 1, planes, 0);
	obs_leave_graphics();
	bfree(pixels);
	return stripes;
}

static void stripes_destroy(void *data)
{
	struct stripes *stripes = data;
	obs_enter_graphics();
	gs_texture_destroy(stripes->tex);
	obs_leave_graphics();
	bfree(stripes);
}

static uint32_t stripes_size(void *data)
{
	(void)data;
	return STRIPES_SIZE;
}

static void stripes_render(void *data, gs_effect_t *effect)
{
	struct stripes *stripes = data;
	(void)effect;
	gs_effect_t *def = obs_get_base_effect(OBS_EFFECT_DEFAULT);
	gs_effect_set_texture(gs_effect_get_param_by_name(def, "image"), stripes->tex);
	while (gs_effect_loop(def, "Draw"))
		gs_draw_sprite(stripes->tex, 0, STRIPES_SIZE, STRIPES_SIZE);
}

/* Lets the video thread drop the references it takes while ticking, then
 * waits for the deferred destroys. */
static void settle(void)
{
	uint32_t start = obs_get_total_frames();
	for (int i = 0; i < 500 && obs_get_total_frames() - start < 2; i++)
		os_sleep_ms(10);
	obs_wait_for_destroy_queue();
}

struct stats {
	double mean;
	int min;
	int max;
	int count;
	double sd;
};

/* Renders the stripes through a scene item scaled by `scale` and rotated
 * by `rot` degrees about the canvas centre, and returns the brightness of the
 * pixels within a disc in the middle of the item (inside the item at any
 * rotation, away from its edges). */
static struct stats render_item(enum obs_scale_type filter, float scale, float rot)
{
	struct stats st = {0.0, 255, 0, 0};

	obs_source_t *stripes = obs_source_create(STRIPES_ID, "stripes", NULL, NULL);
	assert_non_null(stripes);
	obs_scene_t *scene = obs_scene_create_private("rotated scale filter");
	assert_non_null(scene);
	obs_sceneitem_t *item = obs_scene_add(scene, stripes);
	assert_non_null(item);

	struct vec2 pos = {CANVAS_SIZE / 2.0f, CANVAS_SIZE / 2.0f};
	struct vec2 sc = {scale, scale};
	obs_sceneitem_defer_update_begin(item);
	obs_sceneitem_set_alignment(item, OBS_ALIGN_CENTER);
	obs_sceneitem_set_pos(item, &pos);
	obs_sceneitem_set_scale(item, &sc);
	obs_sceneitem_set_rot(item, rot);
	obs_sceneitem_set_scale_filter(item, filter);
	obs_sceneitem_defer_update_end(item);

	uint8_t *data;
	uint32_t linesize;
	double sum = 0.0, sum2 = 0.0;
	double radius = 0.35 * STRIPES_SIZE * scale;

	obs_enter_graphics();
	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	gs_stagesurf_t *stage = gs_stagesurface_create(CANVAS_SIZE, CANVAS_SIZE, GS_RGBA);

	if (gs_texrender_begin(tr, CANVAS_SIZE, CANVAS_SIZE)) {
		struct vec4 clear;
		vec4_zero(&clear);
		gs_clear(GS_CLEAR_COLOR, &clear, 0.0f, 0);
		gs_ortho(0.0f, (float)CANVAS_SIZE, 0.0f, (float)CANVAS_SIZE, -100.0f, 100.0f);
		gs_blend_state_push();
		gs_reset_blend_state();
		obs_source_video_render(obs_scene_get_source(scene));
		gs_blend_state_pop();
		gs_texrender_end(tr);

		gs_stage_texture(stage, gs_texrender_get_texture(tr));
		if (gs_stagesurface_map(stage, &data, &linesize)) {
			for (int y = 0; y < CANVAS_SIZE; y++) {
				const uint8_t *row = data + (size_t)y * linesize;
				for (int x = 0; x < CANVAS_SIZE; x++) {
					double dx = x + 0.5 - CANVAS_SIZE / 2.0;
					double dy = y + 0.5 - CANVAS_SIZE / 2.0;
					if (dx * dx + dy * dy > radius * radius)
						continue;
					int v = row[x * 4];
					sum += v;
					sum2 += (double)v * v;
					if (v < st.min)
						st.min = v;
					if (v > st.max)
						st.max = v;
					st.count++;
				}
			}
			gs_stagesurface_unmap(stage);
		}
	}

	gs_stagesurface_destroy(stage);
	gs_texrender_destroy(tr);
	obs_leave_graphics();

	obs_scene_release(scene);
	obs_source_release(stripes);
	settle();

	if (st.count) {
		st.mean = sum / st.count;
		st.sd = sqrt(fmax(sum2 / st.count - st.mean * st.mean, 0.0));
	}
	return st;
}

static const char *filter_name(enum obs_scale_type filter)
{
	switch (filter) {
	case OBS_SCALE_AREA:
		return "area";
	case OBS_SCALE_BILINEAR:
		return "bilinear";
	case OBS_SCALE_BICUBIC:
		return "bicubic";
	case OBS_SCALE_LANCZOS:
		return "lanczos";
	default:
		return "other";
	}
}

/* Checks that the item, scaled by `scale` with `filter`, is filtered at each
 * rotation in `rots` the way it is at 0 degrees. */
static void check_rotations(enum obs_scale_type filter, float scale, const float *rots, size_t n)
{
	struct stats ref = render_item(filter, scale, 0.0f);
	fprintf(stderr, "%s x%.2f at 0 deg: mean %.1f, sd %.1f, range %d-%d over %d pixels\n", filter_name(filter),
		scale, ref.mean, ref.sd, ref.min, ref.max, ref.count);
	assert_true(ref.count > 100);

	int failures = 0;
	for (size_t i = 0; i < n; i++) {
		struct stats st = render_item(filter, scale, rots[i]);
		bool ok = st.count == ref.count && fabs(st.mean - ref.mean) <= MAX_MEAN_DIFF &&
			  st.sd <= ref.sd + MAX_SD_DIFF;
		fprintf(stderr, "%s x%.2f at %g deg: mean %.1f, sd %.1f, range %d-%d over %d pixels%s\n",
			filter_name(filter), scale, rots[i], st.mean, st.sd, st.min, st.max, st.count,
			ok ? "" : " (not filtered like at 0 deg)");
		if (!ok)
			failures++;
	}

	if (failures) {
		fprintf(stderr, "%s x%.2f: %d rotation(s) not filtered like at 0 deg\n", filter_name(filter), scale,
			failures);
		fail();
	}
}

/* Area from half size up draws through area.effect. Before the fix the item
 * came out black at 90 and 270 degrees, and at 180 degrees too. */
static void area_rotated_90(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const float rots[] = {90.0f, 270.0f, -90.0f, 180.0f};
	check_rotations(OBS_SCALE_AREA, 0.6f, rots, 4);
}

/* Custom rotations: before the fix the footprint shrank with the cosine of
 * the angle, so the item aliased more the closer it got to 90 degrees. */
static void area_rotated_custom(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const float rots[] = {45.0f, 60.0f, 75.0f, -75.0f};
	check_rotations(OBS_SCALE_AREA, 0.6f, rots, 4);
}

/* Area also uses the footprint when it scales up (sharp texels with smooth
 * edges); rotated, it fell back to point sampling or drew shifted texels. */
static void area_upscale_rotated(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const float rots[] = {90.0f, 180.0f, 270.0f};
	check_rotations(OBS_SCALE_AREA, 1.5f, rots, 3);
}

/* Area below half size (here 4x down) draws through
 * bilinear_lowres_scale.effect, like every filter but Point. At exactly 90
 * degrees the collapsed taps happen to land between two stripes and still
 * give grey; just short of it they alias. */
static void area_quarter_rotated(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const float rots[] = {90.0f, 89.0f, 75.0f};
	check_rotations(OBS_SCALE_AREA, 0.25f, rots, 3);
}

static void bilinear_lowres_rotated_90(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const float rots[] = {90.0f, 270.0f, 75.0f};
	check_rotations(OBS_SCALE_BILINEAR, 0.3f, rots, 3);
}

/* Controls: the low-res filter worked at 180 degrees and close enough at 45
 * degrees before the fix, and must still. */
static void bilinear_lowres_rotated_180_45(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	const float rots[] = {180.0f, 45.0f};
	check_rotations(OBS_SCALE_BILINEAR, 0.3f, rots, 2);
}

static int setup(void **state)
{
	(void)state;
	/* obs_startup() needs a display on Linux (run under xvfb-run); the
	 * cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = CANVAS_SIZE,
		.base_height = CANVAS_SIZE,
		.output_width = CANVAS_SIZE,
		.output_height = CANVAS_SIZE,
		.output_format = VIDEO_FORMAT_NV12,
		.colorspace = VIDEO_CS_709,
		.range = VIDEO_RANGE_PARTIAL,
		.scale_type = OBS_SCALE_BICUBIC,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	struct obs_source_info info = {
		.id = STRIPES_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
		.get_name = stripes_name,
		.create = stripes_create,
		.destroy = stripes_destroy,
		.get_width = stripes_size,
		.get_height = stripes_size,
		.video_render = stripes_render,
	};
	obs_register_source(&info);

	/* Let the graphics thread get through its first frames before the
	 * cases render on its context. */
	for (int i = 0; i < 500 && obs_get_total_frames() < 3; i++)
		os_sleep_ms(10);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{

	const struct CMUnitTest tests[] = {
		cmocka_unit_test(bilinear_lowres_rotated_180_45),
		cmocka_unit_test(area_rotated_90),
		cmocka_unit_test(area_rotated_custom),
		cmocka_unit_test(area_upscale_rotated),
		cmocka_unit_test(area_quarter_rotated),
		cmocka_unit_test(bilinear_lowres_rotated_90),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
