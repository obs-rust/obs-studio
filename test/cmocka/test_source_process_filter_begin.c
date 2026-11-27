/* obs_source_process_filter_begin() and
 * obs_source_process_filter_begin_with_color_space() called directly, and
 * from a filter that renders through them (#5831).
 *
 * Every effect filter starts its pass with one of them. This test pins what
 * they do through the real libobs:
 *
 * - a NULL filter: one LOG_DEBUG line naming
 *   obs_source_process_filter_begin_with_color_space, from either, and false;
 * - a filter removed from its parent: one LOG_INFO "no target" line naming
 *   the filter, false, and the bypass flag a bypass begin left cleared;
 * - direct rendering over a plain parent in the space asked for: the
 *   bypass (true, nothing drawn yet; the end draws the parent itself), and
 *   no bypass when the current space is another, or direct rendering is
 *   not allowed: the parent drawn into the filter's texture, and true;
 * - obs_source_process_filter_begin() asks for GS_CS_SRGB;
 * - a zero-size target: the filter is skipped (the parent drawn straight
 *   through) and false; the bypass check comes first;
 * - a texrender of another format is replaced: GS_R8, then GS_RGBA again,
 *   draws the square green again.
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

#define PARENT_ID "process_filter_begin_parent"
#define FILTER_ID "process_filter_begin_filter"
#define FRAME_SIZE 16
#define CANVAS_SIZE 64
#define SQUARE (FRAME_SIZE * FRAME_SIZE)

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

static log_handler_t old_handler;
static void *old_param;

static void capture_start(void)
{
	base_get_log_handler(&old_handler, &old_param);
	memset(&captured, 0, sizeof(captured));
	base_set_log_handler(capture_log, NULL);
}

static void capture_stop(void)
{
	base_set_log_handler(old_handler, old_param);
}

/* ------------------------------------------------------------------------- */
/* sources */

/* The parent's size, set by each case. */
static uint32_t parent_size_value = FRAME_SIZE;

/* How many draws the parent made. */
static struct {
	int draws;
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
	return parent_size_value;
}

/* Draws the green square with whatever effect it is given (the default
 * effect, or the filter's on the bypass path). */
static void parent_render(void *data, gs_effect_t *effect)
{
	(void)data;
	parent_seen.draws++;
	gs_effect_set_texture(gs_effect_get_param_by_name(effect, "image"), green);
	gs_draw_sprite(green, 0, FRAME_SIZE, FRAME_SIZE);
}

enum filter_mode {
	FILTER_IDLE,
	FILTER_BEGIN,
	FILTER_BEGIN_WITH_SPACE,
};

/* How the filter renders, set by each case, and what its begin returned
 * and how many draws the parent had made by then. */
static struct {
	enum filter_mode mode;
	enum gs_color_format format;
	enum gs_color_space space;
	enum obs_allow_direct_render direct;
	int begins;
	bool began;
	int draws_in_begin;
	bool no_end;
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

	filter_setup.begins++;
	if (filter_setup.mode == FILTER_BEGIN)
		filter_setup.began = obs_source_process_filter_begin(filter, filter_setup.format, filter_setup.direct);
	else
		filter_setup.began = obs_source_process_filter_begin_with_color_space(
			filter, filter_setup.format, filter_setup.space, filter_setup.direct);
	filter_setup.draws_in_begin = parent_seen.draws;
	if (!filter_setup.began || filter_setup.no_end)
		return;

	obs_source_process_filter_end(filter, obs_get_base_effect(OBS_EFFECT_DEFAULT), 0, 0);
}

/* ------------------------------------------------------------------------- */
/* rendering */

/* Runs draw(arg) into a cleared CANVAS_SIZE texture of `space` on this
 * thread, and counts the green pixels and the other drawn (not
 * transparent) ones. */
static int render_and_count(void (*draw)(void *), void *arg, enum gs_color_space space, int *other)
{
	int count = 0;
	uint8_t *data;
	uint32_t linesize;

	*other = 0;
	obs_enter_graphics();
	gs_texrender_t *tr = gs_texrender_create(GS_RGBA, GS_ZS_NONE);
	gs_stagesurf_t *stage = gs_stagesurface_create(CANVAS_SIZE, CANVAS_SIZE, GS_RGBA);

	if (gs_texrender_begin_with_color_space(tr, CANVAS_SIZE, CANVAS_SIZE, space)) {
		struct vec4 clear;
		vec4_zero(&clear);
		gs_clear(GS_CLEAR_COLOR, &clear, 0.0f, 0);
		gs_ortho(0.0f, (float)CANVAS_SIZE, 0.0f, (float)CANVAS_SIZE, -100.0f, 100.0f);
		gs_blend_state_push();
		gs_reset_blend_state();
		draw(arg);
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

static void draw_tech_end(void *arg)
{
	obs_source_process_filter_tech_end(arg, obs_get_base_effect(OBS_EFFECT_DEFAULT), 0, 0, NULL);
}

struct pair {
	obs_source_t *parent;
	obs_source_t *filter;
};

static struct pair make_pair(void)
{
	struct pair pair;
	pair.parent = obs_source_create_private(PARENT_ID, "parent", NULL);
	assert_non_null(pair.parent);
	pair.filter = obs_source_create_private(FILTER_ID, "filter", NULL);
	assert_non_null(pair.filter);
	obs_source_filter_add(pair.parent, pair.filter);
	return pair;
}

static void free_pair(struct pair pair)
{
	filter_setup.mode = FILTER_IDLE;
	obs_source_filter_remove(pair.parent, pair.filter);
	obs_source_release(pair.filter);
	obs_source_release(pair.parent);
	obs_wait_for_destroy_queue();
	parent_size_value = FRAME_SIZE;
}

static void reset_seen(enum filter_mode mode, enum gs_color_format format, enum gs_color_space space,
		       enum obs_allow_direct_render direct)
{
	memset(&parent_seen, 0, sizeof(parent_seen));
	filter_setup.mode = mode;
	filter_setup.format = format;
	filter_setup.space = space;
	filter_setup.direct = direct;
	filter_setup.begins = 0;
	filter_setup.began = false;
	filter_setup.draws_in_begin = -1;
	filter_setup.no_end = false;
}

/* Renders the parent (and so its filter) once into a canvas of `space` and
 * returns the green pixels; `other` gets the other drawn ones. */
static int render_parent(struct pair pair, enum gs_color_space space, int *other)
{
	return render_and_count(draw_source, pair.parent, space, other);
}

/* ------------------------------------------------------------------------- */
/* cases */

/* A NULL filter: either begin logs one LOG_DEBUG line naming
 * obs_source_process_filter_begin_with_color_space and the parameter, and
 * returns false before graphics. */
static void null_filter_logs_and_fails(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	capture_start();
	const bool began = obs_source_process_filter_begin(NULL, GS_RGBA, OBS_ALLOW_DIRECT_RENDERING);
	capture_stop();
	assert_false(began);
	assert_int_equal(captured.count, 1);
	assert_int_equal(captured.level, LOG_DEBUG);
	assert_string_equal(captured.last, "obs_source_process_filter_begin_with_color_space: Null 'filter' parameter");

	capture_start();
	const bool began_space =
		obs_source_process_filter_begin_with_color_space(NULL, GS_RGBA, GS_CS_SRGB, OBS_NO_DIRECT_RENDERING);
	capture_stop();
	assert_false(began_space);
	assert_int_equal(captured.count, 1);
	assert_int_equal(captured.level, LOG_DEBUG);
	assert_string_equal(captured.last, "obs_source_process_filter_begin_with_color_space: Null 'filter' parameter");
}

/* A filter removed after a bypass begin: begin clears the bypass flag,
 * logs one LOG_INFO "no target" line with the filter's name and returns
 * false. Re-added, an end with no begin takes the texture path (no texture
 * yet: nothing drawn) instead of the bypass left over. */
static void removed_filter_no_target(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair();
	reset_seen(FILTER_IDLE, GS_RGBA, GS_CS_SRGB, OBS_ALLOW_DIRECT_RENDERING);

	obs_enter_graphics();
	assert_true(obs_source_process_filter_begin(pair.filter, GS_RGBA, OBS_ALLOW_DIRECT_RENDERING));
	obs_leave_graphics();
	assert_int_equal(parent_seen.draws, 0);

	obs_source_filter_remove(pair.parent, pair.filter);

	obs_enter_graphics();
	capture_start();
	const bool began = obs_source_process_filter_begin_with_color_space(pair.filter, GS_RGBA, GS_CS_SRGB,
									    OBS_ALLOW_DIRECT_RENDERING);
	capture_stop();
	obs_leave_graphics();
	assert_false(began);
	assert_int_equal(captured.count, 1);
	assert_int_equal(captured.level, LOG_INFO);
	assert_string_equal(captured.last, "filter 'filter' being processed with no target!");
	assert_int_equal(parent_seen.draws, 0);

	obs_source_filter_add(pair.parent, pair.filter);
	int other;
	int count = render_and_count(draw_tech_end, pair.filter, GS_CS_SRGB, &other);
	fprintf(stderr, "re-added end after removed begin: %d green, %d other, parent drew %d time(s)\n", count, other,
		parent_seen.draws);
	assert_int_equal(count, 0);
	assert_int_equal(other, 0);
	assert_int_equal(parent_seen.draws, 0);
	free_pair(pair);
}

/* Direct rendering over a plain parent in the current space: begin takes
 * the bypass, returns true and draws nothing; the end draws the parent. */
static void bypass_draws_nothing_in_begin(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair();
	reset_seen(FILTER_BEGIN, GS_RGBA, GS_CS_SRGB, OBS_ALLOW_DIRECT_RENDERING);
	int other;
	int count = render_parent(pair, GS_CS_SRGB, &other);
	fprintf(stderr, "bypass: %d green, %d other, parent drew %d time(s), %d in begin\n", count, other,
		parent_seen.draws, filter_setup.draws_in_begin);
	assert_int_equal(filter_setup.begins, 1);
	assert_true(filter_setup.began);
	assert_int_equal(filter_setup.draws_in_begin, 0);
	assert_int_equal(parent_seen.draws, 1);
	assert_int_equal(count, SQUARE);
	assert_int_equal(other, 0);
	free_pair(pair);
}

/* No direct rendering: begin draws the parent into the filter's texture
 * and returns true; the end draws that texture. */
static void texture_path_draws_parent_in_begin(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair();
	reset_seen(FILTER_BEGIN, GS_RGBA, GS_CS_SRGB, OBS_NO_DIRECT_RENDERING);
	int other;
	int count = render_parent(pair, GS_CS_SRGB, &other);
	fprintf(stderr, "texture: %d green, %d other, parent drew %d time(s), %d in begin\n", count, other,
		parent_seen.draws, filter_setup.draws_in_begin);
	assert_true(filter_setup.began);
	assert_int_equal(filter_setup.draws_in_begin, 1);
	assert_int_equal(parent_seen.draws, 1);
	assert_int_equal(count, SQUARE);
	assert_int_equal(other, 0);
	free_pair(pair);
}

/* The filter's begin called straight in the canvas (obs_source_video_render()
 * of a parent that is not sRGB-aware would draw it through an sRGB texture),
 * with no end (outside its parent's render, a bypass end would render the
 * parent's filters again). */
static void draw_filter(void *arg)
{
	filter_setup.no_end = true;
	filter_render(arg, NULL);
}

/* One begin on a fresh pair (no texrender yet: one rendered earlier in the
 * same frame does not begin again) with direct rendering allowed, in a
 * canvas of `canvas`; how many draws the parent made in it. */
static int draws_in_begin(enum filter_mode mode, enum gs_color_space space, enum gs_color_space canvas)
{
	struct pair pair = make_pair();
	int other;
	reset_seen(mode, GS_RGBA, space, OBS_ALLOW_DIRECT_RENDERING);
	render_and_count(draw_filter, pair.filter, canvas, &other);
	assert_int_equal(filter_setup.begins, 1);
	assert_true(filter_setup.began);
	const int draws = filter_setup.draws_in_begin;
	free_pair(pair);
	return draws;
}

/* The space asked for decides the bypass against the current one: a 709
 * extended begin in an sRGB canvas draws the parent into the texture, in a
 * 709 extended canvas it takes the bypass; an sRGB begin in a 709 extended
 * canvas draws into the texture too, and obs_source_process_filter_begin()
 * is the sRGB one. */
static void color_space_passed_through(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	assert_int_equal(draws_in_begin(FILTER_BEGIN_WITH_SPACE, GS_CS_709_EXTENDED, GS_CS_SRGB), 1);
	assert_int_equal(draws_in_begin(FILTER_BEGIN_WITH_SPACE, GS_CS_709_EXTENDED, GS_CS_709_EXTENDED), 0);
	assert_int_equal(draws_in_begin(FILTER_BEGIN_WITH_SPACE, GS_CS_SRGB, GS_CS_709_EXTENDED), 1);
	assert_int_equal(draws_in_begin(FILTER_BEGIN_WITH_SPACE, GS_CS_SRGB, GS_CS_SRGB), 0);
	assert_int_equal(draws_in_begin(FILTER_BEGIN, GS_CS_709_EXTENDED, GS_CS_709_EXTENDED), 1);
	assert_int_equal(draws_in_begin(FILTER_BEGIN, GS_CS_709_EXTENDED, GS_CS_SRGB), 0);
}

/* A zero-size target: with no direct rendering, begin skips the filter
 * (the parent drawn straight through, once) and returns false; with direct
 * rendering the bypass is checked first and taken. */
static void zero_size_skips(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair();
	parent_size_value = 0;
	int other;

	reset_seen(FILTER_BEGIN, GS_RGBA, GS_CS_SRGB, OBS_NO_DIRECT_RENDERING);
	int count = render_parent(pair, GS_CS_SRGB, &other);
	fprintf(stderr, "zero size: %d green, %d other, parent drew %d time(s)\n", count, other, parent_seen.draws);
	assert_false(filter_setup.began);
	assert_int_equal(filter_setup.draws_in_begin, 1);
	assert_int_equal(parent_seen.draws, 1);

	reset_seen(FILTER_BEGIN, GS_RGBA, GS_CS_SRGB, OBS_ALLOW_DIRECT_RENDERING);
	render_parent(pair, GS_CS_SRGB, &other);
	assert_true(filter_setup.began);
	assert_int_equal(filter_setup.draws_in_begin, 0);
	assert_int_equal(parent_seen.draws, 1);
	free_pair(pair);
}

/* A texrender of another format is replaced: GS_R8 keeps only red (the
 * green square comes out black), and GS_RGBA after it draws green again. */
static void format_change_recreates_texrender(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	struct pair pair = make_pair();
	int other;

	reset_seen(FILTER_BEGIN, GS_RGBA, GS_CS_SRGB, OBS_NO_DIRECT_RENDERING);
	assert_int_equal(render_parent(pair, GS_CS_SRGB, &other), SQUARE);
	assert_int_equal(other, 0);

	reset_seen(FILTER_BEGIN, GS_R8, GS_CS_SRGB, OBS_NO_DIRECT_RENDERING);
	int count = render_parent(pair, GS_CS_SRGB, &other);
	fprintf(stderr, "R8: %d green, %d other\n", count, other);
	assert_true(filter_setup.began);
	assert_int_equal(count, 0);
	assert_int_equal(other, SQUARE);

	reset_seen(FILTER_BEGIN, GS_RGBA, GS_CS_SRGB, OBS_NO_DIRECT_RENDERING);
	count = render_parent(pair, GS_CS_SRGB, &other);
	fprintf(stderr, "RGBA again: %d green, %d other\n", count, other);
	assert_int_equal(count, SQUARE);
	assert_int_equal(other, 0);
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

	static uint8_t pixels[SQUARE * 4];
	for (size_t i = 0; i < SQUARE; i++) {
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
	int drawn = render_and_count(draw_green, NULL, GS_CS_SRGB, &other);
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
		cmocka_unit_test(null_filter_logs_and_fails),
		cmocka_unit_test(removed_filter_no_target),
		cmocka_unit_test(bypass_draws_nothing_in_begin),
		cmocka_unit_test(texture_path_draws_parent_in_begin),
		cmocka_unit_test(color_space_passed_through),
		cmocka_unit_test(zero_size_skips),
		cmocka_unit_test(format_change_recreates_texrender),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
