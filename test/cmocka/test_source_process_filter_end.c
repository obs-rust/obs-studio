/* obs_source_process_filter_end() and obs_source_process_filter_tech_end()
 * called directly, and from a filter that renders through them (#5829).
 *
 * Every effect filter finishes its pass with one of them after
 * obs_source_process_filter_begin(). This test pins what they do through
 * the real libobs:
 *
 * - a NULL filter: process_filter_end logs one LOG_DEBUG line, tech_end
 *   returns without one;
 * - after a begin that rendered into the filter's texture, the end draws
 *   that texture; after a begin that chose the direct (bypass) path, the
 *   end draws the parent itself, through the effect;
 * - a NULL technique name is "Draw"; a technique the effect lacks draws
 *   nothing on either path;
 * - the bypass draw runs with linear sRGB set from the filter's
 *   OBS_SOURCE_SRGB flag, and the caller's value is restored after;
 * - the end clears the bypass flag even for a filter removed from its
 *   parent, which draws nothing: re-added, the next end draws no bypass.
 *
 * The parent draws a green FRAME_SIZE square; each case renders into a
 * CANVAS_SIZE texture on the test thread and counts the green pixels. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <obs.h>
#include <graphics/vec4.h>
#include <util/base.h>
#include <util/platform.h>

#define PARENT_ID "process_filter_end_parent"
#define SRGB_PARENT_ID "process_filter_end_srgb_parent"
#define FILTER_ID "process_filter_end_filter"
#define SRGB_FILTER_ID "process_filter_end_srgb_filter"
#define FRAME_SIZE 16
#define CANVAS_SIZE 64

static bool obs_up;
static gs_texture_t *green;

/* ------------------------------------------------------------------------- */
/* the log */

static struct {
	int count;
	int level;
	char last[256];
} captured;

static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	captured.count++;
	captured.level = level;
	vsnprintf(captured.last, sizeof(captured.last), format, args);
}

/* ------------------------------------------------------------------------- */
/* sources */

/* What the parent saw on its last draw, and how many draws. */
static struct {
	int draws;
	bool linear_srgb;
} parent_seen;

static const char *parent_name(void *type_data)
{
	(void)type_data;
	return "parent";
}

static void *parent_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void parent_destroy(void *data)
{
	(void)data;
}

static uint32_t parent_size(void *data)
{
	(void)data;
	return FRAME_SIZE;
}

/* Draws the green square with whatever effect it is given (the default
 * effect, or the filter's on the bypass path). */
static void parent_render(void *data, gs_effect_t *effect)
{
	(void)data;
	parent_seen.draws++;
	parent_seen.linear_srgb = gs_get_linear_srgb();
	gs_effect_set_texture(gs_effect_get_param_by_name(effect, "image"), green);
	gs_draw_sprite(green, 0, FRAME_SIZE, FRAME_SIZE);
}

enum filter_mode {
	FILTER_IDLE,
	FILTER_END,
	FILTER_TECH_END,
};

/* How the filter renders, set by each case. */
static struct {
	enum filter_mode mode;
	enum obs_allow_direct_render direct;
	const char *tech;
	int restore_mismatches;
} filter_setup;

static const char *filter_name(void *type_data)
{
	(void)type_data;
	return "filter";
}

static void *filter_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void filter_destroy(void *data)
{
	(void)data;
}

static void filter_render(void *data, gs_effect_t *effect)
{
	obs_source_t *filter = data;
	(void)effect;
	if (filter_setup.mode == FILTER_IDLE)
		return;

	gs_effect_t *draw = obs_get_base_effect(OBS_EFFECT_DEFAULT);
	if (!obs_source_process_filter_begin(filter, GS_RGBA, filter_setup.direct))
		return;

	const bool before = gs_get_linear_srgb();
	if (filter_setup.mode == FILTER_END)
		obs_source_process_filter_end(filter, draw, 0, 0);
	else
		obs_source_process_filter_tech_end(filter, draw, 0, 0, filter_setup.tech);
	if (gs_get_linear_srgb() != before)
		filter_setup.restore_mismatches++;
}

/* ------------------------------------------------------------------------- */
/* rendering */

/* Runs draw(arg) into a cleared CANVAS_SIZE texture on this thread, with
 * linear sRGB set to linear_srgb, and counts the green pixels and the
 * other drawn (not transparent) ones. */
static int render_and_count(void (*draw)(void *), void *arg, bool linear_srgb, int *other)
{
	int count = 0;
	uint8_t *data;
	uint32_t linesize;

	*other = 0;
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
		const bool previous = gs_set_linear_srgb(linear_srgb);
		draw(arg);
		gs_set_linear_srgb(previous);
		gs_blend_state_pop();
		gs_texrender_end(tr);

		gs_stage_texture(stage, gs_texrender_get_texture(tr));
		if (gs_stagesurface_map(stage, &data, &linesize)) {
			for (int y = 0; y < CANVAS_SIZE; y++) {
				const uint8_t *row = data + (size_t)y * linesize;
				for (int x = 0; x < CANVAS_SIZE; x++) {
					const uint8_t *p = row + x * 4;
					if (p[0] == 0 && p[1] == 255 && p[2] == 0 && p[3] == 255)
						count++;
					else if (p[3] != 0)
						(*other)++;
				}
			}
			gs_stagesurface_unmap(stage);
		}
	}

	gs_stagesurface_destroy(stage);
	gs_texrender_destroy(tr);
	obs_leave_graphics();
	return count;
}

/* The square on its own, through the default effect. */
static void draw_green(void *arg)
{
	(void)arg;
	gs_effect_t *effect = obs_get_base_effect(OBS_EFFECT_DEFAULT);
	gs_effect_set_texture(gs_effect_get_param_by_name(effect, "image"), green);
	while (gs_effect_loop(effect, "Draw"))
		gs_draw_sprite(green, 0, FRAME_SIZE, FRAME_SIZE);
}

static void draw_source(void *arg)
{
	obs_source_video_render(arg);
}

struct tech_end_call {
	obs_source_t *filter;
	const char *tech;
	bool linear_after;
};

static void draw_tech_end(void *arg)
{
	struct tech_end_call *call = arg;
	obs_source_process_filter_tech_end(call->filter, obs_get_base_effect(OBS_EFFECT_DEFAULT), 0, 0, call->tech);
	call->linear_after = gs_get_linear_srgb();
}

struct pair {
	obs_source_t *parent;
	obs_source_t *filter;
};

static struct pair make_pair(const char *parent_id, const char *filter_id)
{
	struct pair pair;
	pair.parent = obs_source_create_private(parent_id, "parent", NULL);
	assert_non_null(pair.parent);
	pair.filter = obs_source_create_private(filter_id, "filter", NULL);
	assert_non_null(pair.filter);
	obs_source_filter_add(pair.parent, pair.filter);
	return pair;
}

static void free_pair(struct pair pair)
{
	obs_source_filter_remove(pair.parent, pair.filter);
	obs_source_release(pair.filter);
	obs_source_release(pair.parent);
	obs_wait_for_destroy_queue();
}

static void reset_seen(enum filter_mode mode, enum obs_allow_direct_render direct, const char *tech)
{
	memset(&parent_seen, 0, sizeof(parent_seen));
	filter_setup.mode = mode;
	filter_setup.direct = direct;
	filter_setup.tech = tech;
	filter_setup.restore_mismatches = 0;
}

/* Renders the parent (and so its filter) once and returns the green
 * pixels. */
static int render_parent(struct pair pair, bool linear_srgb)
{
	int other;
	int count = render_and_count(draw_source, pair.parent, linear_srgb, &other);
	assert_int_equal(other, 0);
	return count;
}

/* ------------------------------------------------------------------------- */
/* cases */

/* A NULL filter: process_filter_end logs one LOG_DEBUG line naming itself
 * and the parameter, and returns before graphics. */
static void end_null_filter_logs(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	memset(&captured, 0, sizeof(captured));
	base_set_log_handler(capture_log, NULL);
	obs_source_process_filter_end(NULL, NULL, 0, 0);
	base_set_log_handler(old_handler, old_param);

	assert_int_equal(captured.count, 1);
	assert_int_equal(captured.level, LOG_DEBUG);
	assert_string_equal(captured.last, "obs_source_process_filter_end: Null 'filter' parameter");
}

/* A NULL filter: tech_end returns silently. */
static void tech_end_null_filter_silent(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	memset(&captured, 0, sizeof(captured));
	base_set_log_handler(capture_log, NULL);
	obs_source_process_filter_tech_end(NULL, NULL, 0, 0, "Draw");
	obs_source_process_filter_tech_end(NULL, NULL, 0, 0, NULL);
	base_set_log_handler(old_handler, old_param);

	assert_int_equal(captured.count, 0);
}

/* No direct rendering: begin draws the parent into the filter's texture,
 * the end draws that texture. */
static void end_draws_texture(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair(PARENT_ID, FILTER_ID);
	reset_seen(FILTER_END, OBS_NO_DIRECT_RENDERING, NULL);
	int count = render_parent(pair, false);
	fprintf(stderr, "texture path: %d green, parent drew %d time(s)\n", count, parent_seen.draws);
	assert_int_equal(count, FRAME_SIZE * FRAME_SIZE);
	assert_int_equal(parent_seen.draws, 1);
	assert_int_equal(filter_setup.restore_mismatches, 0);
	free_pair(pair);
}

/* Direct rendering allowed over a plain parent: begin takes the bypass,
 * and the end draws the parent itself through the effect. */
static void end_draws_bypass(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair(PARENT_ID, FILTER_ID);
	reset_seen(FILTER_END, OBS_ALLOW_DIRECT_RENDERING, NULL);
	int count = render_parent(pair, false);
	fprintf(stderr, "bypass path: %d green, parent drew %d time(s)\n", count, parent_seen.draws);
	assert_int_equal(count, FRAME_SIZE * FRAME_SIZE);
	assert_int_equal(parent_seen.draws, 1);
	assert_int_equal(filter_setup.restore_mismatches, 0);
	free_pair(pair);
}

/* tech_end with a NULL name draws with "Draw", on both paths. */
static void tech_end_null_name_is_draw(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair(PARENT_ID, FILTER_ID);
	reset_seen(FILTER_TECH_END, OBS_NO_DIRECT_RENDERING, NULL);
	assert_int_equal(render_parent(pair, false), FRAME_SIZE * FRAME_SIZE);
	assert_int_equal(parent_seen.draws, 1);
	reset_seen(FILTER_TECH_END, OBS_ALLOW_DIRECT_RENDERING, NULL);
	assert_int_equal(render_parent(pair, false), FRAME_SIZE * FRAME_SIZE);
	assert_int_equal(parent_seen.draws, 1);
	assert_int_equal(filter_setup.restore_mismatches, 0);
	free_pair(pair);
}

/* A technique the effect lacks has no passes: the texture is not drawn,
 * and on the bypass path the parent is never asked to draw. */
static void tech_end_unknown_name_draws_nothing(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair(PARENT_ID, FILTER_ID);
	reset_seen(FILTER_TECH_END, OBS_NO_DIRECT_RENDERING, "NoSuchTechnique");
	assert_int_equal(render_parent(pair, false), 0);
	/* begin drew the parent into the texture */
	assert_int_equal(parent_seen.draws, 1);
	reset_seen(FILTER_TECH_END, OBS_ALLOW_DIRECT_RENDERING, "NoSuchTechnique");
	assert_int_equal(render_parent(pair, false), 0);
	assert_int_equal(parent_seen.draws, 0);
	assert_int_equal(filter_setup.restore_mismatches, 0);
	free_pair(pair);
}

/* The bypass draw runs with linear sRGB set from the filter's flag: an
 * sRGB-aware filter over an sRGB-aware parent draws it with linear sRGB on,
 * though the caller had it off, and a plain pair with it off, though the
 * caller had it on (the plain parent's main render turns it off too). The
 * caller's value is back after the end. */
static void bypass_linear_srgb_from_filter_flags(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair(SRGB_PARENT_ID, SRGB_FILTER_ID);
	reset_seen(FILTER_END, OBS_ALLOW_DIRECT_RENDERING, NULL);
	assert_int_equal(render_parent(pair, false), FRAME_SIZE * FRAME_SIZE);
	assert_int_equal(parent_seen.draws, 1);
	assert_true(parent_seen.linear_srgb);
	assert_int_equal(filter_setup.restore_mismatches, 0);
	free_pair(pair);

	pair = make_pair(PARENT_ID, FILTER_ID);
	reset_seen(FILTER_END, OBS_ALLOW_DIRECT_RENDERING, NULL);
	assert_int_equal(render_parent(pair, true), FRAME_SIZE * FRAME_SIZE);
	assert_int_equal(parent_seen.draws, 1);
	assert_false(parent_seen.linear_srgb);
	assert_int_equal(filter_setup.restore_mismatches, 0);
	free_pair(pair);
}

/* tech_end called directly after a bypass begin draws the parent's chain:
 * with the filter rendering through begin and end there, that is the green
 * square. Linear sRGB is restored to the caller's value either way. */
static void tech_end_after_bypass_begin_draws_parent(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair(PARENT_ID, FILTER_ID);
	reset_seen(FILTER_END, OBS_ALLOW_DIRECT_RENDERING, NULL);

	obs_enter_graphics();
	assert_true(obs_source_process_filter_begin(pair.filter, GS_RGBA, OBS_ALLOW_DIRECT_RENDERING));
	obs_leave_graphics();
	assert_int_equal(parent_seen.draws, 0);

	struct tech_end_call call = {pair.filter, "Draw", false};
	int other;
	int count = render_and_count(draw_tech_end, &call, true, &other);
	fprintf(stderr, "direct end after bypass begin: %d green, parent drew %d time(s)\n", count, parent_seen.draws);
	assert_int_equal(count, FRAME_SIZE * FRAME_SIZE);
	assert_int_equal(other, 0);
	assert_int_equal(parent_seen.draws, 1);
	assert_true(call.linear_after);
	free_pair(pair);
}

/* The end clears the bypass flag before it looks the target and parent up:
 * a filter removed after a bypass begin draws nothing at its end, and once
 * re-added, an end with no begin takes the texture path (no texture yet:
 * nothing drawn) instead of the bypass left over. */
static void removed_filter_end_clears_bypass(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair(PARENT_ID, FILTER_ID);
	reset_seen(FILTER_END, OBS_ALLOW_DIRECT_RENDERING, NULL);

	obs_enter_graphics();
	assert_true(obs_source_process_filter_begin(pair.filter, GS_RGBA, OBS_ALLOW_DIRECT_RENDERING));
	obs_leave_graphics();

	obs_source_filter_remove(pair.parent, pair.filter);

	struct tech_end_call call = {pair.filter, NULL, true};
	int other;
	assert_int_equal(render_and_count(draw_tech_end, &call, false, &other), 0);
	assert_int_equal(other, 0);
	assert_int_equal(parent_seen.draws, 0);
	assert_false(call.linear_after);

	obs_source_filter_add(pair.parent, pair.filter);
	call.linear_after = false;
	int count = render_and_count(draw_tech_end, &call, true, &other);
	fprintf(stderr, "re-added end after removed end: %d green, parent drew %d time(s)\n", count, parent_seen.draws);
	assert_int_equal(count, 0);
	assert_int_equal(other, 0);
	assert_int_equal(parent_seen.draws, 0);
	assert_true(call.linear_after);
	free_pair(pair);
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

	struct obs_source_info parent = {
		.id = PARENT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = parent_name,
		.create = parent_create,
		.destroy = parent_destroy,
		.get_width = parent_size,
		.get_height = parent_size,
		.video_render = parent_render,
	};
	obs_register_source(&parent);

	struct obs_source_info srgb_parent = parent;
	srgb_parent.id = SRGB_PARENT_ID;
	srgb_parent.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_SRGB;
	obs_register_source(&srgb_parent);

	struct obs_source_info filter = {
		.id = FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = filter_name,
		.create = filter_create,
		.destroy = filter_destroy,
		.video_render = filter_render,
	};
	obs_register_source(&filter);

	struct obs_source_info srgb_filter = filter;
	srgb_filter.id = SRGB_FILTER_ID;
	srgb_filter.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_SRGB;
	obs_register_source(&srgb_filter);

	static uint8_t pixels[FRAME_SIZE * FRAME_SIZE * 4];
	for (size_t i = 0; i < FRAME_SIZE * FRAME_SIZE; i++) {
		pixels[i * 4 + 0] = 0;
		pixels[i * 4 + 1] = 255;
		pixels[i * 4 + 2] = 0;
		pixels[i * 4 + 3] = 255;
	}
	const uint8_t *planes[] = {pixels};
	obs_enter_graphics();
	green = gs_texture_create(FRAME_SIZE, FRAME_SIZE, GS_RGBA, 1, planes, 0);
	obs_leave_graphics();
	if (!green) {
		fprintf(stderr, "skipped: no texture\n");
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	/* Let the graphics thread get through its first frames, then draw the
	 * square once on this thread: the first texrender read back after
	 * startup can come back empty, on the C reference too. */
	for (int i = 0; i < 500 && obs_get_total_frames() < 3; i++)
		os_sleep_ms(10);
	int other;
	int drawn = render_and_count(draw_green, NULL, false, &other);
	fprintf(stderr, "warm-up: %d green, %d other\n", drawn, other);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_enter_graphics();
		gs_texture_destroy(green);
		obs_leave_graphics();
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(end_null_filter_logs),
		cmocka_unit_test(tech_end_null_filter_silent),
		cmocka_unit_test(end_draws_texture),
		cmocka_unit_test(end_draws_bypass),
		cmocka_unit_test(tech_end_null_name_is_draw),
		cmocka_unit_test(tech_end_unknown_name_draws_nothing),
		cmocka_unit_test(bypass_linear_srgb_from_filter_flags),
		cmocka_unit_test(tech_end_after_bypass_begin_draws_parent),
		cmocka_unit_test(removed_filter_end_clears_bypass),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
