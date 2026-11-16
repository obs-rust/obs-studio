/*
 * Characterize the deinterlace of libobs/obs-source-deinterlace.c, the part
 * not yet ported: the mode and field-order setters, the frame take
 * (deinterlace_process_last_frame() under async_tick()), the texture update
 * after a render (deinterlace_update_async_video()) and the render
 * (deinterlace_render()). The decisions inside them are pinned by the
 * select, pacing and draw step tests; this pins what the callers do with
 * them on a real source.
 *
 * The core runs video on libobs-opengl, so it needs a display (Linux, under
 * xvfb-run); without one each case skips. An async test source is fed
 * frames through obs_source_output_video(), and the test reads its fields
 * (obs-internal.h) under the locks their writers take: async_mutex for the
 * frame slots and the queue, the graphics context for the textures. It
 * pins:
 * - the mode change: none to blend, blend to yadif, yadif to none, a mode
 *   outside the enum on a source that deinterlaces and on one that does
 *   not, and the same mode twice: the mode read back, the effect (one
 *   per mode, shared by every source, kept by a disable), and whether the
 *   previous textures exist (made by an enable once the source has a
 *   frame, kept by a switch, destroyed by a disable), and a NULL source;
 * - the field order read back, for top, bottom and a value outside the
 *   enum;
 * - the frame take, buffered and unbuffered, in a 2x mode: the current
 *   frame only ever moves forward in timestamp, the previous frame (when
 *   set) is older than the current one, and the queue drains to the last
 *   frame pushed;
 * - a time jump past MAX_TS_VAR: the jumped frame is taken at once, not
 *   ten seconds later;
 * - the update after a render in a 2x mode: the upload and swap reuse the
 *   same two textures, which differ;
 * - the render skip: a deinterlaced source with no frame draws nothing,
 *   in every output frame that shows a probe source drawn in the same
 *   scene (proof the frame is that scene's).
 *
 * Every wait is for the state it checks, with a 10 s bound that only turns a
 * hang into a failure.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include "cmocka-compat.h"

#include "obs-internal.h"
#include <util/platform.h>
#include <util/threading.h>

#define SOURCE_ID "source_deinterlace_test_source"
#define W 64
#define H 64
#define CANVAS_FPS 30
#define WAIT_NS (10ULL * 1000000000ULL)
/* A field at 60i: two a frame at 30 frames a second. */
#define FIELD_NS (1000000000ULL / 60)

static bool obs_up;

/* The probe: a synchronous source that draws a green square of PROBE_SIZE
 * at the scene's top left, away from the centre the checks read. */
#define PROBE_ID "source_deinterlace_probe"
#define PROBE_SIZE 8

/* The output frames the raw video callback saw that show the probe, and how
 * many of those showed anything but black at the centre. */
static pthread_mutex_t watch_mutex = PTHREAD_MUTEX_INITIALIZER;
static size_t watch_probed;
static size_t watch_lit;

static void raw_video(void *param, struct video_data *frame)
{
	(void)param;
	const uint8_t *px = frame->data[0] + (H / 2) * frame->linesize[0] + (W / 2) * 4;
	const uint8_t *corner = frame->data[0] + 2 * frame->linesize[0] + 2 * 4;
	const bool lit = px[0] > 8 || px[1] > 8 || px[2] > 8;
	const bool probe = corner[0] < 40 && corner[1] > 200 && corner[2] < 40;

	pthread_mutex_lock(&watch_mutex);
	if (probe) {
		watch_probed++;
		if (lit)
			watch_lit++;
	}
	pthread_mutex_unlock(&watch_mutex);
}

static const char *test_source_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "source deinterlace test source";
}

static void *test_source_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void test_source_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static struct obs_source_info test_source = {
	.id = SOURCE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_ASYNC_VIDEO,
	.get_name = test_source_name,
	.create = test_source_create,
	.destroy = test_source_destroy,
};

static const char *probe_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "source deinterlace probe";
}

static uint32_t probe_size(void *data)
{
	UNUSED_PARAMETER(data);
	return PROBE_SIZE;
}

static void probe_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(effect);
	gs_effect_t *solid = obs_get_base_effect(OBS_EFFECT_SOLID);
	struct vec4 green;
	vec4_set(&green, 0.0f, 1.0f, 0.0f, 1.0f);
	gs_effect_set_vec4(gs_effect_get_param_by_name(solid, "color"), &green);
	while (gs_effect_loop(solid, "Solid"))
		gs_draw_sprite(NULL, 0, PROBE_SIZE, PROBE_SIZE);
}

static struct obs_source_info probe_source = {
	.id = PROBE_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
	.get_name = probe_name,
	.create = test_source_create,
	.destroy = test_source_destroy,
	.get_width = probe_size,
	.get_height = probe_size,
	.video_render = probe_render,
};

static uint8_t pixels[W * H * 4];

static void push_frame(obs_source_t *source, uint64_t ts)
{
	struct obs_source_frame frame = {
		.data = {pixels},
		.linesize = {W * 4},
		.width = W,
		.height = H,
		.format = VIDEO_FORMAT_BGRA,
		.full_range = true,
		.timestamp = ts,
	};
	obs_source_output_video(source, &frame);
}

struct slots {
	size_t queued;
	bool has_cur;
	bool has_prev;
	uint64_t cur_ts;
	uint64_t prev_ts;
};

static struct slots read_slots(obs_source_t *source)
{
	struct slots s = {0};
	pthread_mutex_lock(&source->async_mutex);
	s.queued = source->async_frames.num;
	s.has_cur = source->cur_async_frame != NULL;
	s.has_prev = source->prev_async_frame != NULL;
	if (s.has_cur)
		s.cur_ts = source->cur_async_frame->timestamp;
	if (s.has_prev)
		s.prev_ts = source->prev_async_frame->timestamp;
	pthread_mutex_unlock(&source->async_mutex);
	return s;
}

struct textures {
	uint32_t width;
	gs_texture_t *cur[MAX_AV_PLANES];
	gs_texture_t *prev[MAX_AV_PLANES];
	gs_texrender_t *prev_texrender;
};

/* The texture fields, as the graphics thread last left them. */
static struct textures read_textures(obs_source_t *source)
{
	struct textures t = {0};
	obs_enter_graphics();
	t.width = source->async_width;
	for (size_t c = 0; c < MAX_AV_PLANES; c++) {
		t.cur[c] = source->async_textures[c];
		t.prev[c] = source->async_prev_textures[c];
	}
	t.prev_texrender = source->async_prev_texrender;
	obs_leave_graphics();
	return t;
}

static obs_source_t *make_source(const char *name)
{
	obs_source_t *source = obs_source_create(SOURCE_ID, name, NULL, NULL);
	assert_non_null(source);
	return source;
}

static void drop_source(obs_source_t *source)
{
	obs_source_remove(source);
	obs_source_release(source);
}

static obs_scene_t *show(obs_source_t *source)
{
	obs_scene_t *scene = obs_scene_create_private("source deinterlace scene");
	obs_scene_add(scene, source);
	obs_set_output_source(0, obs_scene_get_source(scene));
	return scene;
}

static void hide(obs_scene_t *scene)
{
	obs_set_output_source(0, NULL);
	obs_scene_release(scene);
}

/* Pushes a frame at the canvas rate until the source's textures hold one:
 * async_width is set by the first upload. */
static void wait_uploaded(obs_source_t *source)
{
	const uint64_t start = os_gettime_ns();
	uint64_t next = start;
	while (read_textures(source).width != W) {
		if (os_gettime_ns() - start > WAIT_NS)
			fail();
		push_frame(source, next);
		next += 1000000000ULL / CANVAS_FPS;
		os_sleepto_ns(next);
	}
}

/* The effect a fresh source loads for `mode`: the core keeps one per mode,
 * so every source in that mode shares it (obs->video is hidden, so the test
 * finds it through a source). */
static gs_effect_t *mode_effect(enum obs_deinterlace_mode mode)
{
	obs_source_t *probe = make_source("effect probe");
	obs_source_set_deinterlace_mode(probe, mode);
	gs_effect_t *effect = probe->deinterlace_effect;
	drop_source(probe);
	return effect;
}

/* ------------------------------------------------------------------------ */

/* Each mode from none, on a source with no frame: the mode reads back, the
 * effect is loaded, one per mode (each differs, and a second source in the
 * mode gets the same one), and no previous texture is made. */
static void enable_each_mode(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	gs_effect_t *effects[OBS_DEINTERLACE_MODE_YADIF_2X + 1] = {NULL};
	for (int mode = OBS_DEINTERLACE_MODE_DISCARD; mode <= OBS_DEINTERLACE_MODE_YADIF_2X; mode++) {
		obs_source_t *source = make_source("enable each mode");
		obs_source_set_deinterlace_mode(source, (enum obs_deinterlace_mode)mode);

		assert_int_equal(source->deinterlace_mode, mode);
		assert_non_null(source->deinterlace_effect);
		effects[mode] = source->deinterlace_effect;
		assert_ptr_equal(mode_effect((enum obs_deinterlace_mode)mode), effects[mode]);
		for (int other = OBS_DEINTERLACE_MODE_DISCARD; other < mode; other++)
			assert_ptr_not_equal(effects[other], effects[mode]);
		struct textures t = read_textures(source);
		assert_null(t.prev[0]);
		assert_null(t.prev_texrender);
		drop_source(source);
	}
}

/* None to blend, blend to yadif, yadif to none, then a mode outside the
 * enum on each side, and the same mode twice, on a source that has drawn a
 * frame. */
static void mode_changes(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *source = make_source("mode changes");
	obs_scene_t *scene = show(source);
	wait_uploaded(source);

	struct textures t = read_textures(source);
	assert_non_null(t.cur[0]);
	assert_null(t.prev[0]);

	/* none to blend: enable, which makes the previous textures */
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_BLEND);
	assert_int_equal(source->deinterlace_mode, OBS_DEINTERLACE_MODE_BLEND);
	assert_ptr_equal(source->deinterlace_effect, mode_effect(OBS_DEINTERLACE_MODE_BLEND));
	t = read_textures(source);
	assert_non_null(t.prev[0]);
	assert_null(t.prev[1]);
	assert_null(t.prev[2]);
	gs_texture_t *made = t.prev[0];

	/* blend to blend: nothing */
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_BLEND);
	assert_int_equal(source->deinterlace_mode, OBS_DEINTERLACE_MODE_BLEND);
	assert_ptr_equal(read_textures(source).prev[0], made);

	/* blend to yadif: switch, the effect only */
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_YADIF);
	assert_int_equal(source->deinterlace_mode, OBS_DEINTERLACE_MODE_YADIF);
	assert_ptr_equal(source->deinterlace_effect, mode_effect(OBS_DEINTERLACE_MODE_YADIF));
	assert_ptr_equal(read_textures(source).prev[0], made);

	/* yadif to none: disable destroys the previous textures, keeps the
	 * effect and the current textures */
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_DISABLE);
	assert_int_equal(source->deinterlace_mode, OBS_DEINTERLACE_MODE_DISABLE);
	assert_ptr_equal(source->deinterlace_effect, mode_effect(OBS_DEINTERLACE_MODE_YADIF));
	t = read_textures(source);
	assert_null(t.prev[0]);
	assert_null(t.prev[1]);
	assert_null(t.prev[2]);
	assert_null(t.prev_texrender);
	assert_non_null(t.cur[0]);

	/* none to a mode outside the enum: nothing */
	obs_source_set_deinterlace_mode(source, (enum obs_deinterlace_mode)99);
	assert_int_equal(source->deinterlace_mode, OBS_DEINTERLACE_MODE_DISABLE);
	assert_null(read_textures(source).prev[0]);

	/* linear 2x to a mode outside the enum: disable */
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_LINEAR_2X);
	assert_ptr_equal(source->deinterlace_effect, mode_effect(OBS_DEINTERLACE_MODE_LINEAR_2X));
	assert_non_null(read_textures(source).prev[0]);
	obs_source_set_deinterlace_mode(source, (enum obs_deinterlace_mode) - 1);
	assert_int_equal(source->deinterlace_mode, OBS_DEINTERLACE_MODE_DISABLE);
	assert_ptr_equal(source->deinterlace_effect, mode_effect(OBS_DEINTERLACE_MODE_LINEAR_2X));
	assert_null(read_textures(source).prev[0]);

	hide(scene);
	drop_source(source);
}

static void field_order(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *source = make_source("field order");
	obs_source_set_deinterlace_field_order(source, OBS_DEINTERLACE_FIELD_ORDER_BOTTOM);
	assert_false(source->deinterlace_top_first);
	obs_source_set_deinterlace_field_order(source, OBS_DEINTERLACE_FIELD_ORDER_TOP);
	assert_true(source->deinterlace_top_first);
	obs_source_set_deinterlace_field_order(source, (enum obs_deinterlace_field_order)7);
	assert_false(source->deinterlace_top_first);
	assert_int_equal(obs_source_get_deinterlace_field_order(source), OBS_DEINTERLACE_FIELD_ORDER_BOTTOM);
	drop_source(source);

	/* A NULL source is refused, with nothing to read. */
	obs_source_set_deinterlace_field_order(NULL, OBS_DEINTERLACE_FIELD_ORDER_TOP);
	obs_source_set_deinterlace_mode(NULL, OBS_DEINTERLACE_MODE_BLEND);
}

/* Pushes `count` frames a field apart, the first at `first`, each when it
 * is due, and follows the frame slots until the last is current and the
 * queue is empty. */
static void follow_take(obs_source_t *source, size_t count, uint64_t first)
{
	uint64_t last_cur = 0;
	size_t moves = 0;
	const uint64_t last_ts = first + (count - 1) * FIELD_NS;
	const uint64_t start = os_gettime_ns();
	size_t pushed = 0;

	for (;;) {
		const uint64_t now = os_gettime_ns();
		while (pushed < count && first + pushed * FIELD_NS <= now + FIELD_NS) {
			push_frame(source, first + pushed * FIELD_NS);
			pushed++;
		}

		struct slots s = read_slots(source);
		if (s.has_cur) {
			if (s.cur_ts < last_cur) {
				fprintf(stderr, "current frame went back: %llu after %llu\n",
					(unsigned long long)s.cur_ts, (unsigned long long)last_cur);
				fail();
			}
			if (s.cur_ts != last_cur)
				moves++;
			last_cur = s.cur_ts;
			if (s.has_prev && s.prev_ts >= s.cur_ts) {
				fprintf(stderr, "previous frame %llu not older than current %llu\n",
					(unsigned long long)s.prev_ts, (unsigned long long)s.cur_ts);
				fail();
			}
		}
		if (pushed == count && s.queued == 0 && s.has_cur && s.cur_ts == last_ts)
			break;
		if (os_gettime_ns() - start > WAIT_NS) {
			fprintf(stderr, "after %zu pushed: %zu queued, current %llu, want %llu\n", pushed, s.queued,
				(unsigned long long)s.cur_ts, (unsigned long long)last_ts);
			fail();
		}
		os_sleep_ms(1);
	}
	assert_true(moves >= 2);
}

static void take_buffered(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *source = make_source("take buffered");
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_BLEND_2X);
	follow_take(source, 24, os_gettime_ns());
	drop_source(source);
}

static void take_unbuffered(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *source = make_source("take unbuffered");
	obs_source_set_async_unbuffered(source, true);
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_YADIF_2X);
	follow_take(source, 24, os_gettime_ns());
	drop_source(source);
}

/* A buffered source whose timestamps jump ten seconds ahead: the frame after
 * the jump is taken at once (MAX_TS_VAR), not when the clock reaches it. */
static void take_after_time_jump(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *source = make_source("time jump");
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_BLEND_2X);
	const uint64_t first = os_gettime_ns();
	follow_take(source, 6, first);

	const uint64_t jumped = first + 10ULL * 1000000000ULL;
	push_frame(source, jumped);
	push_frame(source, jumped + FIELD_NS);
	const uint64_t start = os_gettime_ns();
	for (;;) {
		struct slots s = read_slots(source);
		if (s.has_cur && s.cur_ts >= jumped)
			break;
		if (os_gettime_ns() - start > 5ULL * 1000000000ULL) {
			fprintf(stderr, "the frame after the jump was not taken: current %llu\n",
				(unsigned long long)s.cur_ts);
			fail();
		}
		os_sleep_ms(1);
	}
	drop_source(source);
}

/* A shown source in blend 2x: every render's update uploads into, or swaps,
 * the same two textures. */
static void update_reuses_two_textures(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *source = make_source("update");
	obs_scene_t *scene = show(source);
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_BLEND_2X);
	wait_uploaded(source);

	gs_texture_t *seen[2] = {NULL, NULL};
	size_t samples = 0;
	const uint64_t start = os_gettime_ns();
	uint64_t next = start;
	while (samples < 40) {
		if (os_gettime_ns() - start > WAIT_NS)
			fail();
		push_frame(source, next);
		next += FIELD_NS;
		os_sleepto_ns(next);

		struct textures t = read_textures(source);
		assert_non_null(t.cur[0]);
		assert_non_null(t.prev[0]);
		assert_ptr_not_equal(t.cur[0], t.prev[0]);
		if (!seen[0]) {
			seen[0] = t.cur[0];
			seen[1] = t.prev[0];
		}
		assert_true((t.cur[0] == seen[0] && t.prev[0] == seen[1]) ||
			    (t.cur[0] == seen[1] && t.prev[0] == seen[0]));
		samples++;
	}

	hide(scene);
	drop_source(source);
}

/* A shown source in yadif with no frame: the render skips, so the centre
 * stays black. The probe, drawn in the same scene, marks the output frames
 * that are this scene's; only those are checked, and the wait is for
 * enough of them. */
static void render_skips_without_texture(void **state)
{
	(void)state;
	if (!obs_up)
		skip();

	obs_source_t *source = make_source("render skip");
	obs_source_set_deinterlace_mode(source, OBS_DEINTERLACE_MODE_YADIF);
	obs_source_t *probe = obs_source_create(PROBE_ID, "render skip probe", NULL, NULL);
	assert_non_null(probe);

	pthread_mutex_lock(&watch_mutex);
	watch_probed = 0;
	watch_lit = 0;
	pthread_mutex_unlock(&watch_mutex);

	obs_scene_t *scene = show(source);
	obs_scene_add(scene, probe);

	const uint64_t start = os_gettime_ns();
	for (;;) {
		pthread_mutex_lock(&watch_mutex);
		const size_t probed = watch_probed;
		pthread_mutex_unlock(&watch_mutex);
		if (probed >= 10)
			break;
		if (os_gettime_ns() - start > WAIT_NS) {
			fprintf(stderr, "only %zu output frames showed the probe\n", probed);
			fail();
		}
		os_sleep_ms(5);
	}

	pthread_mutex_lock(&watch_mutex);
	const size_t lit = watch_lit;
	const size_t probed = watch_probed;
	pthread_mutex_unlock(&watch_mutex);
	if (lit)
		fprintf(stderr, "%zu of %zu probed output frames were lit at the centre\n", lit, probed);
	assert_int_equal(lit, 0);
	assert_int_equal(read_textures(source).width, 0);

	hide(scene);
	drop_source(probe);
	drop_source(source);
}

static int setup(void **state)
{
	(void)state;
	for (size_t i = 0; i < sizeof(pixels); i += 4) {
		pixels[i + 0] = 40;
		pixels[i + 1] = 160;
		pixels[i + 2] = 220;
		pixels[i + 3] = 255;
	}

	/* obs_startup() needs a display on Linux (run under xvfb-run); the
	 * cases skip without one. */
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = CANVAS_FPS,
		.fps_den = 1,
		.base_width = W,
		.base_height = H,
		.output_width = W,
		.output_height = H,
		.output_format = VIDEO_FORMAT_BGRA,
		.colorspace = VIDEO_CS_SRGB,
		.range = VIDEO_RANGE_FULL,
		.scale_type = OBS_SCALE_POINT,
	};
	int ret = obs_reset_video(&ovi);
	if (ret != OBS_VIDEO_SUCCESS) {
		fprintf(stderr, "skipped: obs_reset_video failed: %d\n", ret);
		obs_shutdown();
		obs_up = false;
		return 0;
	}

	obs_register_source(&test_source);
	obs_register_source(&probe_source);
	obs_add_raw_video_callback(NULL, raw_video, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (obs_up) {
		obs_remove_raw_video_callback(raw_video, NULL);
		obs_shutdown();
	}
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(enable_each_mode),
		cmocka_unit_test(mode_changes),
		cmocka_unit_test(field_order),
		cmocka_unit_test(take_buffered),
		cmocka_unit_test(take_unbuffered),
		cmocka_unit_test(take_after_time_jump),
		cmocka_unit_test(update_reuses_two_textures),
		cmocka_unit_test(render_skips_without_texture),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
