/* obs_source_skip_video_filter() called directly, and from a filter that
 * skips itself (#5827).
 *
 * test_filter_skip_removed covers a filter with no parent or target, and an
 * attached filter over a custom draw parent (the main render), a plain
 * parent (the default render) and another filter (the target's render).
 * This test pins the rest of what the function does:
 *
 * - a NULL filter logs one LOG_DEBUG line and returns;
 * - over an async video parent it draws the parent's current frame (the
 *   async video render), also when the parent is custom draw too but has no
 *   video_render;
 * - over an async parent with deinterlacing on it draws through the
 *   deinterlacer (deinterlace_render()).
 *
 * The async parents are shown on output channel 0 while frames are pushed,
 * so the graphics thread takes and uploads them as in OBS; once the filter
 * has skipped over a frame, the test renders the parent on its own thread
 * and checks the drawn pixels and the filter's render count. */

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
#include <util/threading.h>

#define ASYNC_PARENT_ID "skip_video_filter_async_parent"
#define ASYNC_CUSTOM_PARENT_ID "skip_video_filter_async_custom_parent"
#define SKIP_FILTER_ID "skip_video_filter_skip_filter"
#define FRAME_SIZE 16
#define CANVAS_SIZE 64
#define PUSH_LIMIT_NS 5000000000ULL

static bool obs_up;

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

static const char *async_parent_name(void *type_data)
{
	(void)type_data;
	return "async parent";
}

static void *async_parent_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void async_parent_destroy(void *data)
{
	(void)data;
}

/* The filter skips itself on every render. */
struct skip_filter {
	obs_source_t *source;
	volatile long renders;
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
	obs_source_skip_video_filter(filter->source);
	os_atomic_inc_long(&filter->renders);
}

static struct skip_filter *filter_data(obs_source_t *filter)
{
	return obs_obj_get_data(filter);
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
	int green;
	int red;
	int other;
};

/* Renders the source into a CANVAS_SIZE texture on the calling thread and
 * counts its green, red and other drawn (not transparent) pixels. */
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
					if (p[0] == 0 && p[1] == 255 && p[2] == 0 && p[3] == 255)
						count.green++;
					else if (p[0] == 255 && p[1] == 0 && p[2] == 0 && p[3] == 255)
						count.red++;
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

/* An async parent with one skipping filter, shown on channel 0 while frames
 * of green even rows and red odd rows are pushed until the filter has skipped over one and the parent
 * has a size; then rendered on this thread. */
static struct pixel_count skip_over_async(const char *parent_id, enum obs_deinterlace_mode mode, long *renders)
{
	static uint8_t pixels[FRAME_SIZE * FRAME_SIZE * 4];
	for (size_t i = 0; i < FRAME_SIZE * FRAME_SIZE; i++) {
		const bool odd = (i / FRAME_SIZE) % 2 == 1;
		pixels[i * 4 + 0] = odd ? 255 : 0;
		pixels[i * 4 + 1] = odd ? 0 : 255;
		pixels[i * 4 + 2] = 0;
		pixels[i * 4 + 3] = 255;
	}

	obs_source_t *parent = obs_source_create_private(parent_id, "parent", NULL);
	assert_non_null(parent);
	obs_source_set_deinterlace_mode(parent, mode);
	obs_source_t *filter = obs_source_create_private(SKIP_FILTER_ID, "skips", NULL);
	assert_non_null(filter);
	obs_source_filter_add(parent, filter);
	struct skip_filter *fd = filter_data(filter);

	obs_set_output_source(0, parent);
	const uint64_t start = os_gettime_ns();
	for (uint64_t n = 0;; n++) {
		const uint64_t ts = start + n * 33333333ULL;
		if (ts - start > PUSH_LIMIT_NS)
			break;
		struct obs_source_frame frame = {
			.data = {pixels},
			.linesize = {FRAME_SIZE * 4},
			.width = FRAME_SIZE,
			.height = FRAME_SIZE,
			.format = VIDEO_FORMAT_RGBA,
			.full_range = true,
			.timestamp = ts,
		};
		os_sleepto_ns(ts);
		obs_source_output_video(parent, &frame);
		/* a few frames past the first size, so the deinterlacer has
		 * its previous frame too */
		if (n > 10 && obs_source_get_width(parent) == FRAME_SIZE && os_atomic_load_long(&fd->renders) > 3)
			break;
	}

	long before = os_atomic_load_long(&fd->renders);
	struct pixel_count count = render_and_count(parent);
	*renders = os_atomic_load_long(&fd->renders) - before;

	obs_set_output_source(0, NULL);
	obs_source_output_video(parent, NULL);
	obs_source_filter_remove(parent, filter);
	obs_source_release(filter);
	obs_source_release(parent);
	settle();
	return count;
}

/* ------------------------------------------------------------------------- */
/* cases */

/* A NULL filter: one LOG_DEBUG line naming the function and the parameter,
 * nothing drawn (called outside the graphics context, it returns before it
 * touches graphics). */
static void null_filter_logs_and_returns(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	memset(&captured, 0, sizeof(captured));
	base_set_log_handler(capture_log, NULL);
	obs_source_skip_video_filter(NULL);
	base_set_log_handler(old_handler, old_param);

	assert_int_equal(captured.count, 1);
	assert_int_equal(captured.level, LOG_DEBUG);
	assert_string_equal(captured.last, "obs_source_skip_video_filter: Null 'filter' parameter");
}

/* Over an async parent the skip draws the parent's frame. */
static void skip_draws_async_frame(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	long renders = 0;
	struct pixel_count count = skip_over_async(ASYNC_PARENT_ID, OBS_DEINTERLACE_MODE_DISABLE, &renders);
	fprintf(stderr, "async parent: %d green, %d red, %d other, filter rendered %ld time(s)\n", count.green,
		count.red, count.other, renders);
	assert_int_equal(renders, 1);
	assert_int_equal(count.green, FRAME_SIZE * FRAME_SIZE / 2);
	assert_int_equal(count.red, FRAME_SIZE * FRAME_SIZE / 2);
	assert_int_equal(count.other, 0);
}

/* Custom draw as well, but no video_render: still the async frame. */
static void skip_draws_async_custom_frame(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	long renders = 0;
	struct pixel_count count = skip_over_async(ASYNC_CUSTOM_PARENT_ID, OBS_DEINTERLACE_MODE_DISABLE, &renders);
	fprintf(stderr, "async custom draw parent: %d green, %d red, %d other, filter rendered %ld time(s)\n",
		count.green, count.red, count.other, renders);
	assert_int_equal(renders, 1);
	assert_int_equal(count.green, FRAME_SIZE * FRAME_SIZE / 2);
	assert_int_equal(count.red, FRAME_SIZE * FRAME_SIZE / 2);
	assert_int_equal(count.other, 0);
}

/* With deinterlacing on, the skip draws through the deinterlacer: Discard
 * keeps one field and doubles its lines, so every row shows the odd (red)
 * rows, where the plain async draw shows both. */
static void skip_draws_deinterlaced_frame(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	long renders = 0;
	struct pixel_count count = skip_over_async(ASYNC_PARENT_ID, OBS_DEINTERLACE_MODE_DISCARD, &renders);
	fprintf(stderr, "deinterlaced async parent: %d green, %d red, %d other, filter rendered %ld time(s)\n",
		count.green, count.red, count.other, renders);
	assert_int_equal(renders, 1);
	assert_int_equal(count.green, 0);
	assert_int_equal(count.red, FRAME_SIZE * FRAME_SIZE);
	assert_int_equal(count.other, 0);
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

	struct obs_source_info async_parent = {
		.id = ASYNC_PARENT_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_ASYNC_VIDEO,
		.get_name = async_parent_name,
		.create = async_parent_create,
		.destroy = async_parent_destroy,
	};
	obs_register_source(&async_parent);

	struct obs_source_info async_custom_parent = async_parent;
	async_custom_parent.id = ASYNC_CUSTOM_PARENT_ID;
	async_custom_parent.output_flags = OBS_SOURCE_ASYNC_VIDEO | OBS_SOURCE_CUSTOM_DRAW;
	obs_register_source(&async_custom_parent);

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
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(null_filter_logs_and_returns),
		cmocka_unit_test(skip_draws_async_frame),
		cmocka_unit_test(skip_draws_async_custom_frame),
		cmocka_unit_test(skip_draws_deinterlaced_frame),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
