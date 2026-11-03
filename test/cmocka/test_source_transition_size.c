/*
 * Characterization of the transition sizing and tick in
 * libobs/obs-source-transition.c (obs-rust/obs-studio#3580), before they move
 * to their own file and are ported: get_cx() and get_cy(),
 * recalculate_transition_matrix(), recalculate_transition_matrices(),
 * recalculate_transition_size() and obs_transition_tick().
 *
 * test_transition_scale_type pins the matrix of each scale type as a start
 * builds it, and the clock rules have their own steps tests. These cases pin
 * what the sizing does through the real libobs:
 * - the actual size (obs_source_get_width() and _height() of a transition)
 *   is the larger width and the larger height of the two children, 0 with
 *   none, and the tick follows a child's resize and an emptied slot;
 * - a size set with obs_transition_set_size() is what the transition
 *   renders at (the size the render2 callback gets), while
 *   obs_source_get_width() still reports the children's size
 *   (characterized, not endorsed);
 * - the tick rebuilds each slot's matrix from the child's size, the scale
 *   type and the alignment;
 * - in manual mode the tick snaps the value to the target with a torque of
 *   0, steps towards it with a torque, and reaches it at once when the
 *   clamp is larger than the step.
 * The tick's reset of the two render targets is not visible from outside.
 *
 * The tick runs on the graphics thread, once per frame, so each case waits
 * for the frame that shows what it checks, with a 5 s bound that only turns
 * a hang into a failure. Transitions take core video for their render
 * targets, so the test only builds on Linux and runs under xvfb-run; without
 * a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <graphics/matrix4.h>
#include <util/platform.h>

#define CHILD_ID "test_tr_size_child"
#define TRANSITION_ID "test_tr_size_transition"
#define WAIT_NS 5000000000ULL

static bool obs_up;

/* The matrix the last child render ran with. */
static struct matrix4 drawn;
static int draws;

struct child {
	uint32_t cx;
	uint32_t cy;
};

static const char *child_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "size child";
}

static void child_update(void *data, obs_data_t *settings)
{
	struct child *c = data;
	c->cx = (uint32_t)obs_data_get_int(settings, "cx");
	c->cy = (uint32_t)obs_data_get_int(settings, "cy");
}

static void *child_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	struct child *c = bzalloc(sizeof(*c));
	child_update(c, settings);
	return c;
}

static void child_destroy(void *data)
{
	bfree(data);
}

static uint32_t child_width(void *data)
{
	return ((struct child *)data)->cx;
}

static uint32_t child_height(void *data)
{
	return ((struct child *)data)->cy;
}

static void child_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(effect);
	gs_matrix_get(&drawn);
	draws++;
}

static struct obs_source_info child_info = {
	.id = CHILD_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
	.get_name = child_name,
	.create = child_create,
	.destroy = child_destroy,
	.update = child_update,
	.get_width = child_width,
	.get_height = child_height,
	.video_render = child_render,
};

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "size transition";
}

static void *tr_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(settings);
	return source;
}

static void tr_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static float mix_a(void *data, float t)
{
	UNUSED_PARAMETER(data);
	return 1.0f - t;
}

static float mix_b(void *data, float t)
{
	UNUSED_PARAMETER(data);
	return t;
}

static bool tr_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
			    size_t channels, size_t sample_rate)
{
	return obs_transition_audio_render(data, ts_out, audio, mixers, channels, sample_rate, mix_a, mix_b);
}

static struct obs_source_info transition_info = {
	.id = TRANSITION_ID,
	.type = OBS_SOURCE_TYPE_TRANSITION,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = tr_name,
	.create = tr_create,
	.destroy = tr_destroy,
	.audio_render = tr_audio_render,
};

/* The sources of the running case, released by case_teardown if a failed
 * assertion ended the case first. */
static struct {
	obs_source_t *tr;
	obs_source_t *child[2];
} live;

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (live.tr)
		obs_transition_clear(live.tr);
	obs_source_release(live.tr);
	obs_source_release(live.child[0]);
	obs_source_release(live.child[1]);
	memset(&live, 0, sizeof(live));
	return 0;
}

static void skip_without_core(void)
{
	if (!obs_up)
		skip();
}

static obs_source_t *make_transition(const char *name)
{
	live.tr = obs_source_create_private(TRANSITION_ID, name, NULL);
	assert_non_null(live.tr);
	return live.tr;
}

static obs_source_t *make_child(size_t i, const char *name, uint32_t cx, uint32_t cy)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "cx", cx);
	obs_data_set_int(settings, "cy", cy);
	live.child[i] = obs_source_create_private(CHILD_ID, name, settings);
	obs_data_release(settings);
	assert_non_null(live.child[i]);
	return live.child[i];
}

/* Resize child i. A video source applies its settings on its next tick,
 * so the transition sees the new size from a tick after that. */
static void resize_child(size_t i, uint32_t cx, uint32_t cy)
{
	obs_data_t *settings = obs_data_create();
	obs_data_set_int(settings, "cx", cx);
	obs_data_set_int(settings, "cy", cy);
	obs_source_update(live.child[i], settings);
	obs_data_release(settings);
}

static bool size_is(obs_source_t *tr, uint32_t cx, uint32_t cy)
{
	return obs_source_get_width(tr) == cx && obs_source_get_height(tr) == cy;
}

/* Wait for the tick that gives `tr` the actual size cx by cy. */
static void wait_for_size(obs_source_t *tr, uint32_t cx, uint32_t cy)
{
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	while (!size_is(tr, cx, cy) && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	if (!size_is(tr, cx, cy)) {
		fprintf(stderr, "size %ux%u, expected %ux%u\n", obs_source_get_width(tr), obs_source_get_height(tr), cx,
			cy);
		fail();
	}
}

/* The actual size is the larger width and the larger height of the two
 * children, 0x0 with none. obs_transition_start() recalculates it at once
 * on a transition with no size set; obs_transition_set() leaves it to the
 * tick, which also follows a child that changes size and a slot that
 * empties. */
static void actual_size_is_the_larger_child(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("larger");
	assert_true(size_is(tr, 0, 0));

	make_child(0, "larger A", 16, 24);
	make_child(1, "larger B", 40, 8);
	obs_transition_set(tr, live.child[0]);
	wait_for_size(tr, 16, 24);
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, live.child[1]));
	assert_true(size_is(tr, 40, 24));

	/* The tick follows a child's resize: the larger of each again. */
	resize_child(0, 48, 4);
	wait_for_size(tr, 48, 8);
	resize_child(1, 8, 2);
	wait_for_size(tr, 48, 4);

	/* Empty slots: 0x0 at the next tick. */
	obs_transition_clear(tr);
	wait_for_size(tr, 0, 0);
}

/* The size the render2 callback got from the last render. */
static uint32_t rendered_cx, rendered_cy;
static int callbacks;

static void record_size(void *data, gs_texture_t *a, gs_texture_t *b, float t, uint32_t cx, uint32_t cy)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(a);
	UNUSED_PARAMETER(b);
	UNUSED_PARAMETER(t);
	rendered_cx = cx;
	rendered_cy = cy;
	callbacks++;
}

static void render_size(obs_source_t *tr, uint32_t *cx, uint32_t *cy)
{
	rendered_cx = rendered_cy = 0;
	callbacks = 0;
	obs_enter_graphics();
	obs_transition_video_render2(tr, record_size, NULL);
	obs_leave_graphics();
	assert_int_equal(callbacks, 1);
	*cx = rendered_cx;
	*cy = rendered_cy;
}

/* get_cx() and get_cy(): a transition renders at the size set with
 * obs_transition_set_size(), and at its actual size without one. The
 * actual size, which obs_source_get_width() and _height() report, still
 * follows the children when a size is set (characterized, not endorsed). */
static void a_set_size_is_the_render_size(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	uint32_t cx, cy;

	obs_source_t *tr = make_transition("render size");
	make_child(0, "render size A", 16, 24);
	make_child(1, "render size B", 8, 32);
	obs_transition_set(tr, live.child[0]);
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, live.child[1]));
	render_size(tr, &cx, &cy);
	assert_int_equal(cx, 16);
	assert_int_equal(cy, 32);

	obs_transition_set_size(tr, 64, 48);
	render_size(tr, &cx, &cy);
	assert_int_equal(cx, 64);
	assert_int_equal(cy, 48);
	assert_true(size_is(tr, 16, 32));

	/* One dimension set: the other is the actual one. */
	obs_transition_set_size(tr, 0, 48);
	render_size(tr, &cx, &cy);
	assert_int_equal(cx, 16);
	assert_int_equal(cy, 48);

	/* With a size set, the tick still recalculates the actual size. */
	obs_transition_set_size(tr, 64, 48);
	resize_child(1, 20, 4);
	wait_for_size(tr, 20, 24);
	render_size(tr, &cx, &cy);
	assert_int_equal(cx, 64);
	assert_int_equal(cy, 48);
}

/* The matrix the child in slot `target` of `tr` is drawn with. */
static void draw_matrix(obs_source_t *tr, enum obs_transition_target target, struct matrix4 *out)
{
	draws = 0;
	obs_enter_graphics();
	gs_matrix_push();
	gs_matrix_identity();
	bool rendered = obs_transition_video_render_direct(tr, target);
	gs_matrix_pop();
	obs_leave_graphics();
	UNUSED_PARAMETER(rendered);
	assert_int_equal(draws, 1);
	matrix4_copy(out, &drawn);
}

static bool row_is(const struct vec4 *row, float x, float y, float z, float w)
{
	return row->x == x && row->y == y && row->z == z && row->w == w;
}

static bool matrix_is(const struct matrix4 *m, float sx, float sy, float px, float py)
{
	return row_is(&m->x, sx, 0.0f, 0.0f, 0.0f) && row_is(&m->y, 0.0f, sy, 0.0f, 0.0f) &&
	       row_is(&m->z, 0.0f, 0.0f, 1.0f, 0.0f) && row_is(&m->t, px, py, 0.0f, 1.0f);
}

/* Wait for the tick that draws slot `target` with scale (sx, sy) at
 * (px, py). */
static void wait_for_matrix(obs_source_t *tr, enum obs_transition_target target, float sx, float sy, float px, float py)
{
	struct matrix4 m;
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	draw_matrix(tr, target, &m);
	while (!matrix_is(&m, sx, sy, px, py) && os_gettime_ns() < deadline) {
		os_sleep_ms(5);
		draw_matrix(tr, target, &m);
	}
	if (!matrix_is(&m, sx, sy, px, py)) {
		fprintf(stderr, "slot %d drawn with scale (%g, %g) at (%g, %g), expected scale (%g, %g) at (%g, %g)\n",
			(int)target, m.x.x, m.y.y, m.t.x, m.t.y, sx, sy, px, py);
		fail();
	}
}

/* The tick rebuilds each slot's matrix: from the child's size, the scale
 * type and the alignment, in a transition of 64x64. A size set before the
 * child keeps obs_transition_set() from building it, so the first matrix
 * comes from a tick. */
static void tick_rebuilds_the_matrices(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("matrices");
	obs_transition_set_size(tr, 64, 64);
	obs_transition_set_alignment(tr, OBS_ALIGN_CENTER);
	make_child(0, "matrices A", 32, 16);
	make_child(1, "matrices B", 16, 32);
	obs_transition_set(tr, live.child[0]);

	/* MAX_ONLY: a child that fits is not scaled, and is centred. */
	wait_for_matrix(tr, OBS_TRANSITION_SOURCE_A, 1.0f, 1.0f, 16.0f, 24.0f);

	/* A resized child. */
	resize_child(0, 16, 16);
	wait_for_matrix(tr, OBS_TRANSITION_SOURCE_A, 1.0f, 1.0f, 24.0f, 24.0f);

	/* A child larger than the transition is scaled down to fit. */
	resize_child(0, 128, 64);
	wait_for_matrix(tr, OBS_TRANSITION_SOURCE_A, 0.5f, 0.5f, 0.0f, 16.0f);

	/* The scale type and the alignment are read on each tick. */
	resize_child(0, 16, 8);
	obs_transition_set_scale_type(tr, OBS_TRANSITION_SCALE_ASPECT);
	wait_for_matrix(tr, OBS_TRANSITION_SOURCE_A, 4.0f, 4.0f, 0.0f, 16.0f);
	obs_transition_set_scale_type(tr, OBS_TRANSITION_SCALE_STRETCH);
	wait_for_matrix(tr, OBS_TRANSITION_SOURCE_A, 4.0f, 8.0f, 0.0f, 0.0f);
	obs_transition_set_scale_type(tr, OBS_TRANSITION_SCALE_MAX_ONLY);
	obs_transition_set_alignment(tr, OBS_ALIGN_RIGHT | OBS_ALIGN_BOTTOM);
	wait_for_matrix(tr, OBS_TRANSITION_SOURCE_A, 1.0f, 1.0f, 48.0f, 56.0f);
	obs_transition_set_alignment(tr, OBS_ALIGN_LEFT | OBS_ALIGN_TOP);
	wait_for_matrix(tr, OBS_TRANSITION_SOURCE_A, 1.0f, 1.0f, 0.0f, 0.0f);

	/* Slot B has its own matrix. */
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_AUTO, 60000, live.child[1]));
	obs_transition_set_alignment(tr, OBS_ALIGN_CENTER);
	wait_for_matrix(tr, OBS_TRANSITION_SOURCE_B, 1.0f, 1.0f, 24.0f, 16.0f);
	resize_child(1, 32, 128);
	wait_for_matrix(tr, OBS_TRANSITION_SOURCE_B, 0.5f, 0.5f, 24.0f, 0.0f);
}

/* Wait for the tick that moves the manual value away from `from`. */
static float wait_for_change(obs_source_t *tr, float from)
{
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	float t = obs_transition_get_time(tr);
	while (t == from && os_gettime_ns() < deadline) {
		os_sleep_ms(5);
		t = obs_transition_get_time(tr);
	}
	if (t == from) {
		fprintf(stderr, "the manual value stayed at %g\n", from);
		fail();
	}
	return t;
}

/* Wait for the tick that takes the manual value to `want`. */
static void wait_for_value(obs_source_t *tr, float want)
{
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	while (obs_transition_get_time(tr) != want && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	if (obs_transition_get_time(tr) != want) {
		fprintf(stderr, "the manual value is %g, expected %g\n", obs_transition_get_time(tr), want);
		fail();
	}
}

/* A manual transition (a T-bar drag): each tick moves the value towards
 * the target set with obs_transition_set_manual_time(). A torque of 0 snaps
 * it there; a torque steps it by (target - value) * torque * seconds, so a
 * small torque only starts on its way; a clamp larger than that step takes
 * it all the way. */
static void manual_tick_moves_the_value(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("manual");
	make_child(0, "manual A", 16, 16);
	make_child(1, "manual B", 16, 16);
	obs_transition_set(tr, live.child[0]);
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, live.child[1]));
	assert_true(obs_transition_get_time(tr) == 0.0f);

	/* Torque 0: snap. */
	obs_transition_set_manual_torque(tr, 0.0f, 0.0f);
	obs_transition_set_manual_time(tr, 0.25f);
	assert_true(wait_for_change(tr, 0.0f) == 0.25f);

	/* Torque 0.1, no clamp: a step of 0.075 a second, and each one less
	 * as the value nears the target. */
	obs_transition_set_manual_torque(tr, 0.1f, 0.0f);
	obs_transition_set_manual_time(tr, 1.0f);
	float t = wait_for_change(tr, 0.25f);
	assert_true(t > 0.25f && t < 1.0f);
	float next = wait_for_change(tr, t);
	assert_true(next > t && next < 1.0f);

	/* A clamp larger than the step: at the target at once. A tick can
	 * come between the two calls and take the value to the old target
	 * first, so this waits for the new one; without the clamp, a torque
	 * of 0.1 would take more than a minute to get there. Downwards, from
	 * a value at its target, the first tick lands on the new one. */
	obs_transition_set_manual_torque(tr, 0.1f, 1000.0f);
	obs_transition_set_manual_time(tr, 0.75f);
	wait_for_value(tr, 0.75f);
	obs_transition_set_manual_time(tr, 0.5f);
	assert_true(wait_for_change(tr, 0.75f) == 0.5f);
	assert_true(obs_transition_is_active(tr));
}

static int group_setup(void **state)
{
	UNUSED_PARAMETER(state);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (!obs_up) {
		fprintf(stderr, "skipped: obs_startup failed (no display?)\n");
		return 0;
	}

	struct obs_video_info ovi = {
		.graphics_module = GRAPHICS_MODULE_PATH,
		.fps_num = 30,
		.fps_den = 1,
		.base_width = 64,
		.base_height = 64,
		.output_width = 64,
		.output_height = 64,
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

	obs_register_source(&child_info);
	obs_register_source(&transition_info);
	return 0;
}

static int group_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(actual_size_is_the_larger_child, case_teardown),
		cmocka_unit_test_teardown(a_set_size_is_the_render_size, case_teardown),
		cmocka_unit_test_teardown(tick_rebuilds_the_matrices, case_teardown),
		cmocka_unit_test_teardown(manual_tick_moves_the_value, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
