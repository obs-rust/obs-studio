/* obs_source_skip_video_filter() on a filter with no parent (#2160).
 *
 * A filter that has nothing to draw (a shader that failed to compile, an
 * effect that is disabled, a size of 0) calls obs_source_skip_video_filter()
 * from its video_render to draw the source below it. The function read the
 * parent's output flags without checking the parent, so a filter that had
 * been removed from its source crashed on a NULL parent:
 * obs_source_filter_remove() clears the filter's parent and target, and the
 * graphics thread renders the filter chain from a reference it took before
 * the removal. obs_source_process_filter_begin() and the other filter helpers
 * already return when there is no parent or target.
 *
 * The removal has to happen while the filter renders. Racing a second thread
 * against the graphics thread would not be deterministic, so the test filter
 * removes itself inside its own video_render: obs_source_render_filters()
 * drops the parent's filter_mutex before it renders the first filter, so
 * obs_source_filter_remove() takes the mutex, clears the parent and target
 * at once, and the skip that follows sees them NULL, exactly as it does when
 * the Filters dialog, a script or obs-websocket removes the filter from
 * another thread at that moment. The parent is rendered on output channel 0,
 * by the graphics thread, as in OBS.
 *
 * The other cases: a filter that was never attached calls the skip directly,
 * and an attached filter that skips still draws its parent (both routes of a
 * filter whose target is its parent, and a filter whose target is another
 * filter), checked on the pixels and on the parent's render count. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <stdlib.h>

#include <obs.h>
#include <graphics/vec4.h>
#include <util/platform.h>
#include <util/threading.h>

#define CUSTOM_PARENT_ID "filter_skip_removed_custom_parent"
#define PLAIN_PARENT_ID "filter_skip_removed_plain_parent"
#define SKIP_FILTER_ID "filter_skip_removed_skip_filter"
#define PARENT_SIZE 16
#define CANVAS_SIZE 64

static bool obs_up;

/* Parents: one draws itself (custom draw) in red; the other draws a green
 * texture with the effect libobs passes it. */
struct parent {
	gs_texture_t *tex;
	volatile long renders;
};

static const char *parent_name(void *type_data)
{
	(void)type_data;
	return "parent";
}

static void *parent_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	struct parent *parent = bzalloc(sizeof(*parent));
	uint8_t *pixels = bmalloc(PARENT_SIZE * PARENT_SIZE * 4);
	for (size_t i = 0; i < PARENT_SIZE * PARENT_SIZE; i++) {
		pixels[i * 4 + 0] = 0;
		pixels[i * 4 + 1] = 255;
		pixels[i * 4 + 2] = 0;
		pixels[i * 4 + 3] = 255;
	}
	const uint8_t *planes[] = {pixels};
	obs_enter_graphics();
	parent->tex = gs_texture_create(PARENT_SIZE, PARENT_SIZE, GS_RGBA, 1, planes, 0);
	obs_leave_graphics();
	bfree(pixels);
	return parent;
}

static void parent_destroy(void *data)
{
	struct parent *parent = data;
	obs_enter_graphics();
	gs_texture_destroy(parent->tex);
	obs_leave_graphics();
	bfree(parent);
}

static uint32_t parent_size(void *data)
{
	(void)data;
	return PARENT_SIZE;
}

static void custom_parent_render(void *data, gs_effect_t *effect)
{
	struct parent *parent = data;
	(void)effect;
	os_atomic_inc_long(&parent->renders);

	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	struct vec4 red;
	vec4_set(&red, 1.0f, 0.0f, 0.0f, 1.0f);
	gs_effect_set_vec4(gs_effect_get_param_by_name(solid, "color"), &red);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, PARENT_SIZE, PARENT_SIZE);
}

static void plain_parent_render(void *data, gs_effect_t *effect)
{
	struct parent *parent = data;
	os_atomic_inc_long(&parent->renders);

	gs_effect_set_texture(gs_effect_get_param_by_name(effect, "image"), parent->tex);
	gs_draw_sprite(parent->tex, 0, PARENT_SIZE, PARENT_SIZE);
}

/* The filter skips itself on every render. With remove_self set it first
 * removes itself from its parent, as if the user removed it at that moment. */
struct skip_filter {
	obs_source_t *source;
	bool remove_self;
	volatile long renders;
	volatile long skips_returned;
};

static const char *skip_filter_name(void *type_data)
{
	(void)type_data;
	return "skip filter";
}

static void *skip_filter_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	struct skip_filter *filter = bzalloc(sizeof(*filter));
	filter->source = source;
	return filter;
}

static void skip_filter_destroy(void *data)
{
	bfree(data);
}

static void skip_filter_render(void *data, gs_effect_t *effect)
{
	struct skip_filter *filter = data;
	(void)effect;
	os_atomic_inc_long(&filter->renders);

	if (filter->remove_self) {
		obs_source_t *parent = obs_filter_get_parent(filter->source);
		if (parent)
			obs_source_filter_remove(parent, filter->source);
	}

	obs_source_skip_video_filter(filter->source);
	os_atomic_inc_long(&filter->skips_returned);
}

static struct skip_filter *filter_data(obs_source_t *filter)
{
	return obs_obj_get_data(filter);
}

static struct parent *parent_data(obs_source_t *parent)
{
	return obs_obj_get_data(parent);
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

struct pixel_count {
	int red;
	int green;
	int other;
};

/* Renders the source into a CANVAS_SIZE texture on the calling thread and
 * counts its red, green and other drawn (not transparent) pixels. */
static struct pixel_count render_and_count(obs_source_t *source)
{
	struct pixel_count count = {0, 0, 0};
	uint8_t *data;
	uint32_t linesize;

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
		obs_source_video_render(source);
		gs_blend_state_pop();
		gs_texrender_end(tr);

		gs_stage_texture(stage, gs_texrender_get_texture(tr));
		if (gs_stagesurface_map(stage, &data, &linesize)) {
			for (int y = 0; y < CANVAS_SIZE; y++) {
				const uint8_t *row = data + (size_t)y * linesize;
				for (int x = 0; x < CANVAS_SIZE; x++) {
					const uint8_t *p = row + x * 4;
					if (p[0] == 255 && p[1] == 0 && p[2] == 0 && p[3] == 255)
						count.red++;
					else if (p[0] == 0 && p[1] == 255 && p[2] == 0 && p[3] == 255)
						count.green++;
					else if (p[3] != 0)
						count.other++;
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

/* A filter removes itself from its parent inside its own video_render, while
 * the graphics thread renders the parent on the output, then skips. Before
 * the fix the skip read the cleared parent and crashed. */
static void filter_removed_during_render_skips(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *parent = obs_source_create_private(CUSTOM_PARENT_ID, "parent", NULL);
	assert_non_null(parent);
	obs_source_t *filter = obs_source_create_private(SKIP_FILTER_ID, "removes itself", NULL);
	assert_non_null(filter);
	struct skip_filter *fd = filter_data(filter);
	fd->remove_self = true;
	obs_source_filter_add(parent, filter);
	assert_ptr_equal(obs_filter_get_parent(filter), parent);

	obs_set_output_source(0, parent);
	for (int i = 0; i < 500 && os_atomic_load_long(&fd->skips_returned) == 0; i++)
		os_sleep_ms(10);
	obs_set_output_source(0, NULL);
	settle();

	fprintf(stderr, "removed filter: %ld render(s), %ld skip(s) returned, parent rendered %ld time(s)\n",
		os_atomic_load_long(&fd->renders), os_atomic_load_long(&fd->skips_returned),
		os_atomic_load_long(&parent_data(parent)->renders));
	assert_int_equal(os_atomic_load_long(&fd->renders), 1);
	assert_int_equal(os_atomic_load_long(&fd->skips_returned), 1);
	assert_null(obs_filter_get_parent(filter));
	assert_null(obs_filter_get_target(filter));
	assert_int_equal(obs_source_filter_count(parent), 0);

	obs_source_release(filter);
	obs_source_release(parent);
	settle();
}

/* A filter that was never added to a source has no parent or target. */
static void unattached_filter_skips(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *filter = obs_source_create_private(SKIP_FILTER_ID, "never attached", NULL);
	assert_non_null(filter);
	assert_null(obs_filter_get_parent(filter));
	assert_null(obs_filter_get_target(filter));

	/* Called outside the graphics context: with no parent there is
	 * nothing to draw, so it must return before it touches graphics. */
	obs_source_skip_video_filter(filter);

	/* Rendered on its own, it reaches the skip from its video_render. */
	struct pixel_count count = render_and_count(filter);
	assert_int_equal(os_atomic_load_long(&filter_data(filter)->renders), 1);
	assert_int_equal(os_atomic_load_long(&filter_data(filter)->skips_returned), 1);
	assert_int_equal(count.red + count.green + count.other, 0);

	obs_source_release(filter);
	settle();
}

/* An attached filter that skips draws its parent, for each route. */
static void check_skip_draws_parent(const char *parent_id, int filters, bool red)
{
	obs_source_t *parent = obs_source_create_private(parent_id, "parent", NULL);
	assert_non_null(parent);
	obs_source_t *f[2] = {NULL, NULL};
	for (int i = 0; i < filters; i++) {
		f[i] = obs_source_create_private(SKIP_FILTER_ID, "skips", NULL);
		assert_non_null(f[i]);
		obs_source_filter_add(parent, f[i]);
	}

	struct pixel_count count = render_and_count(parent);
	long parent_renders = os_atomic_load_long(&parent_data(parent)->renders);
	fprintf(stderr, "%s with %d filter(s): %d red, %d green, %d other; parent rendered %ld time(s)\n", parent_id,
		filters, count.red, count.green, count.other, parent_renders);

	assert_int_equal(parent_renders, 1);
	for (int i = 0; i < filters; i++) {
		assert_int_equal(os_atomic_load_long(&filter_data(f[i])->renders), 1);
		assert_int_equal(os_atomic_load_long(&filter_data(f[i])->skips_returned), 1);
	}
	if ((red ? count.red : count.green) != PARENT_SIZE * PARENT_SIZE || count.other != 0 ||
	    (red ? count.green : count.red) != 0) {
		fprintf(stderr, "expected the parent's %d pixels %s and nothing else\n", PARENT_SIZE * PARENT_SIZE,
			red ? "red" : "green");
		fail();
	}

	for (int i = 0; i < filters; i++) {
		obs_source_filter_remove(parent, f[i]);
		obs_source_release(f[i]);
	}
	obs_source_release(parent);
	settle();
}

/* Target is the parent, which draws itself: obs_source_main_render(). */
static void attached_skip_draws_custom_parent(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_skip_draws_parent(CUSTOM_PARENT_ID, 1, true);
}

/* Target is the parent, drawn with the default effect. */
static void attached_skip_draws_plain_parent(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_skip_draws_parent(PLAIN_PARENT_ID, 1, false);
}

/* The top filter's target is the filter below it: obs_source_video_render(). */
static void attached_skip_chain_draws_parent(void **state)
{
	(void)state;
	if (!obs_up)
		skip();
	check_skip_draws_parent(PLAIN_PARENT_ID, 2, false);
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

	struct obs_source_info custom_parent = {
		.id = CUSTOM_PARENT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
		.get_name = parent_name,
		.create = parent_create,
		.destroy = parent_destroy,
		.get_width = parent_size,
		.get_height = parent_size,
		.video_render = custom_parent_render,
	};
	obs_register_source(&custom_parent);

	struct obs_source_info plain_parent = custom_parent;
	plain_parent.id = PLAIN_PARENT_ID;
	plain_parent.output_flags = OBS_SOURCE_VIDEO;
	plain_parent.video_render = plain_parent_render;
	obs_register_source(&plain_parent);

	struct obs_source_info skip_filter = {
		.id = SKIP_FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = skip_filter_name,
		.create = skip_filter_create,
		.destroy = skip_filter_destroy,
		.video_render = skip_filter_render,
	};
	obs_register_source(&skip_filter);

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
	/* The removal case runs last: without the fix it crashes on the
	 * graphics thread, which cmocka cannot recover from. */
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(attached_skip_draws_custom_parent),
		cmocka_unit_test(attached_skip_draws_plain_parent),
		cmocka_unit_test(attached_skip_chain_draws_parent),
		cmocka_unit_test(unattached_filter_skips),
		cmocka_unit_test(filter_removed_during_render_skips),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
