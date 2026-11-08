/*
 * Characterization of the transition video render in
 * libobs/obs-source-transition.c (obs-rust/obs-studio#4036), before it
 * moves to its own file and is ported: obs_transition_video_render2(),
 * obs_transition_video_render_direct() and
 * obs_transition_video_get_color_space(), with get_texture(),
 * copy_transition_state(), render_child() and mix_spaces() they use.
 *
 * The source_transition_render and source_transition_clock parity tests pin
 * the rules on plain values, test_source_transition_stop pins the stop of an
 * auto transition, and test_source_transition_size the render size and the
 * matrices. These cases pin, through the real libobs, with a transition type
 * whose draw callback records what it is given and children that count their
 * renders and report a color space:
 * - render2's draw for each state: idle, it draws slot A, with or without a
 *   callback; transitioning with a callback, both children render into the
 *   transition's textures and the callback gets the transition's data, both
 *   textures, the time and the size; transitioning without a callback, it
 *   draws slot B (the audio transitions too);
 * - with the texture lock busy (a callback that renders the same transition
 *   again: render2 holds the lock across the callback, so the nested
 *   trylock fails), the nested render draws slot B without the callback,
 *   and the next render takes the callback again;
 * - the placeholder texture for an empty slot, both slots empty included;
 * - the textures' format following the mixed color space, re-created when it
 *   changes (sRGB is RGBA, 16-bit and extended are RGBA16F);
 * - the video stop in render2: at time 1.0 the video ends with the video stop
 *   signal only (the audio runs on, and render2 counts the transition as
 *   active), slots unchanged; once, however often it renders;
 * - the direct render: the child for each target, idle and transitioning,
 *   and its return; at time 1.0 on a transition that is not active, the video
 *   and audio both end and the transition stops (slot B becomes slot A, both
 *   signals and nothing more on the next render);
 * - obs_transition_video_get_color_space(): sRGB with both slots empty, the
 *   mix of slot A's space and sRGB idle, the mix of both while transitioning,
 *   for every pair of the four spaces, each child asked with the three
 *   preferred spaces.
 *
 * Only valid transitions: obs_transition_video_get_color_space() does not
 * check for NULL. Transitions take core video for their render targets, so
 * the test only builds on Linux and runs under xvfb-run; without a display
 * the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/platform.h>

#define CHILD_ID "test_tr_video_child"
#define TRANSITION_ID "test_tr_video_transition"
#define WAIT_NS 5000000000ULL
#define SIZE 32

static bool obs_up;

/* What each child is and how often it rendered. */
struct child_rec {
	long renders;
	enum gs_color_space space;
	size_t asked_count;
	enum gs_color_space asked[3];
};

static struct child_rec recs[3];

static const char *child_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "video child";
}

static void *child_create(obs_data_t *settings, obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return &recs[obs_data_get_int(settings, "idx")];
}

static void child_destroy(void *data)
{
	UNUSED_PARAMETER(data);
}

static uint32_t child_size(void *data)
{
	UNUSED_PARAMETER(data);
	return 16;
}

static void child_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(effect);
	struct child_rec *rec = data;
	rec->renders++;
}

static enum gs_color_space child_color_space(void *data, size_t count, const enum gs_color_space *preferred_spaces)
{
	struct child_rec *rec = data;
	rec->asked_count = count;
	for (size_t i = 0; i < count && i < 3; i++)
		rec->asked[i] = preferred_spaces[i];
	return rec->space;
}

static struct obs_source_info child_info = {
	.id = CHILD_ID,
	.type = OBS_SOURCE_TYPE_INPUT,
	.output_flags = OBS_SOURCE_VIDEO | OBS_SOURCE_CUSTOM_DRAW,
	.get_name = child_name,
	.create = child_create,
	.destroy = child_destroy,
	.get_width = child_size,
	.get_height = child_size,
	.video_render = child_render,
	.video_get_color_space = child_color_space,
};

/* The sources of the running case, released by case_teardown if a failed
 * assertion ended the case first. */
static struct {
	obs_source_t *tr;
	obs_source_t *child[3];
	gs_texture_t *placeholder;
} live;

/* What the draw callback was given. */
static struct {
	long calls;
	void *data;
	gs_texture_t *a;
	gs_texture_t *b;
	float t;
	uint32_t cx;
	uint32_t cy;
	enum gs_color_format a_format;
	enum gs_color_format b_format;
	uint32_t a_width;
	bool nest;
} drawn;

static long stop_signals;
static long video_stop_signals;

static void draw(void *data, gs_texture_t *a, gs_texture_t *b, float t, uint32_t cx, uint32_t cy);

static void render_again(void)
{
	obs_transition_video_render2(live.tr, draw, live.placeholder);
}

static void draw(void *data, gs_texture_t *a, gs_texture_t *b, float t, uint32_t cx, uint32_t cy)
{
	drawn.calls++;
	drawn.data = data;
	drawn.a = a;
	drawn.b = b;
	drawn.t = t;
	drawn.cx = cx;
	drawn.cy = cy;
	drawn.a_format = a && a != live.placeholder ? gs_texture_get_color_format(a) : GS_UNKNOWN;
	drawn.b_format = b && b != live.placeholder ? gs_texture_get_color_format(b) : GS_UNKNOWN;
	drawn.a_width = a ? gs_texture_get_width(a) : 0;
	if (drawn.nest) {
		/* The texture lock is held here: a nested render's trylock
		 * fails. */
		drawn.nest = false;
		render_again();
	}
}

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "video transition";
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

static void tr_video_render(void *data, gs_effect_t *effect)
{
	UNUSED_PARAMETER(effect);
	obs_transition_video_render(data, NULL);
}

static enum gs_color_space tr_color_space(void *data, size_t count, const enum gs_color_space *preferred_spaces)
{
	UNUSED_PARAMETER(count);
	UNUSED_PARAMETER(preferred_spaces);
	return obs_transition_video_get_color_space(data);
}

/* Registration requires one; nothing outputs these transitions, so the audio
 * thread never calls it. */
static bool tr_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
			    size_t channels, size_t sample_rate)
{
	UNUSED_PARAMETER(data);
	UNUSED_PARAMETER(audio);
	UNUSED_PARAMETER(mixers);
	UNUSED_PARAMETER(channels);
	UNUSED_PARAMETER(sample_rate);
	*ts_out = 0;
	return false;
}

static struct obs_source_info transition_info = {
	.id = TRANSITION_ID,
	.type = OBS_SOURCE_TYPE_TRANSITION,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = tr_name,
	.create = tr_create,
	.destroy = tr_destroy,
	.video_render = tr_video_render,
	.audio_render = tr_audio_render,
	.video_get_color_space = tr_color_space,
};

static void on_stop(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	assert_ptr_equal(calldata_ptr(cd, "source"), live.tr);
	stop_signals++;
}

static void on_video_stop(void *param, calldata_t *cd)
{
	UNUSED_PARAMETER(param);
	assert_ptr_equal(calldata_ptr(cd, "source"), live.tr);
	video_stop_signals++;
}

static int case_setup(void **state)
{
	UNUSED_PARAMETER(state);
	memset(recs, 0, sizeof(recs));
	memset(&drawn, 0, sizeof(drawn));
	stop_signals = 0;
	video_stop_signals = 0;
	return 0;
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (live.tr) {
		signal_handler_t *sh = obs_source_get_signal_handler(live.tr);
		signal_handler_disconnect(sh, "transition_stop", on_stop, NULL);
		signal_handler_disconnect(sh, "transition_video_stop", on_video_stop, NULL);
		obs_transition_clear(live.tr);
	}
	obs_source_release(live.tr);
	for (size_t i = 0; i < 3; i++)
		obs_source_release(live.child[i]);
	if (live.placeholder) {
		obs_enter_graphics();
		gs_texture_destroy(live.placeholder);
		obs_leave_graphics();
	}
	memset(&live, 0, sizeof(live));
	return 0;
}

static void skip_without_core(void)
{
	if (!obs_up)
		skip();
}

/* A transition of SIZE x SIZE holding child 0 in slot A, children 1 and 2 to
 * start towards, its stop signals counted, and a placeholder texture. */
static obs_source_t *make_transition(const char *name)
{
	for (size_t i = 0; i < 3; i++) {
		char child_name_buf[64];
		snprintf(child_name_buf, sizeof(child_name_buf), "%s %c", name, (char)('A' + i));
		obs_data_t *settings = obs_data_create();
		obs_data_set_int(settings, "idx", (long long)i);
		live.child[i] = obs_source_create_private(CHILD_ID, child_name_buf, settings);
		obs_data_release(settings);
		assert_non_null(live.child[i]);
		recs[i].space = GS_CS_SRGB;
	}
	live.tr = obs_source_create(TRANSITION_ID, name, NULL, NULL);
	assert_non_null(live.tr);
	signal_handler_t *sh = obs_source_get_signal_handler(live.tr);
	signal_handler_connect(sh, "transition_stop", on_stop, NULL);
	signal_handler_connect(sh, "transition_video_stop", on_video_stop, NULL);
	obs_transition_set_size(live.tr, SIZE, SIZE);
	obs_transition_set(live.tr, live.child[0]);

	obs_enter_graphics();
	live.placeholder = gs_texture_create(4, 4, GS_RGBA, 1, NULL, 0);
	obs_leave_graphics();
	assert_non_null(live.placeholder);
	return live.tr;
}

static void check_renders(long a, long b, long c)
{
	assert_int_equal(recs[0].renders, a);
	assert_int_equal(recs[1].renders, b);
	assert_int_equal(recs[2].renders, c);
}

static void check_slots(obs_source_t *tr, obs_source_t *a, obs_source_t *b, bool active)
{
	obs_source_t *got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_A);
	assert_ptr_equal(got, a);
	obs_source_release(got);
	got = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_B);
	assert_ptr_equal(got, b);
	obs_source_release(got);
	assert_true(obs_transition_is_active(tr) == active);
}

static void frame_changes(void)
{
	uint64_t frame = obs_get_video_frame_time();
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	while (obs_get_video_frame_time() == frame && os_gettime_ns() < deadline)
		os_sleep_ms(2);
	assert_true(obs_get_video_frame_time() != frame);
}

/* Wait for a video tick: it resets the transition's texrenders, which draw
 * once until then, so each render2 renders its children again. The frame
 * time moves at the end of a loop, before its tick, so a tick has run once
 * it has moved twice. */
static void next_frame(void)
{
	frame_changes();
	frame_changes();
}

static void render2(obs_source_t *tr, bool with_callback)
{
	next_frame();
	obs_enter_graphics();
	obs_transition_video_render2(tr, with_callback ? draw : NULL, live.placeholder);
	obs_leave_graphics();
}

static bool render_direct(obs_source_t *tr, enum obs_transition_target target)
{
	obs_enter_graphics();
	gs_matrix_push();
	gs_matrix_identity();
	bool transitioning = obs_transition_video_render_direct(tr, target);
	gs_matrix_pop();
	obs_leave_graphics();
	return transitioning;
}

/* Wait until the transition's time is `t` (the video tick snaps the manual
 * time). */
static bool time_is(obs_source_t *tr, float t)
{
	uint64_t deadline = os_gettime_ns() + WAIT_NS;
	while (obs_transition_get_time(tr) != t && os_gettime_ns() < deadline)
		os_sleep_ms(5);
	return obs_transition_get_time(tr) == t;
}

/* A manual transition from child 0 to child 1, held at 0.5. */
static void start_halfway(obs_source_t *tr)
{
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, live.child[1]));
	obs_transition_set_manual_time(tr, 0.5f);
	assert_true(time_is(tr, 0.5f));
}

/* Idle, render2 draws slot A, with or without a callback, and calls no
 * callback. */
static void idle_draws_a(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("idle");

	render2(tr, true);
	check_renders(1, 0, 0);
	assert_int_equal(drawn.calls, 0);
	render2(tr, false);
	check_renders(2, 0, 0);
	assert_int_equal(drawn.calls, 0);
	assert_int_equal(video_stop_signals, 0);
	assert_int_equal(stop_signals, 0);
}

/* Transitioning with a callback: each child renders once into its texture,
 * and the callback gets the data, both textures (not the placeholder), the
 * time and the size. */
static void transitioning_calls_back(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("callback");
	start_halfway(tr);

	render2(tr, true);
	check_renders(1, 1, 0);
	assert_int_equal(drawn.calls, 1);
	assert_ptr_equal(drawn.data, tr);
	assert_non_null(drawn.a);
	assert_non_null(drawn.b);
	assert_ptr_not_equal(drawn.a, live.placeholder);
	assert_ptr_not_equal(drawn.b, live.placeholder);
	assert_ptr_not_equal(drawn.a, drawn.b);
	assert_true(drawn.t == 0.5f);
	assert_int_equal(drawn.cx, SIZE);
	assert_int_equal(drawn.cy, SIZE);
	assert_int_equal(drawn.a_width, SIZE);

	/* The same textures the next time. */
	gs_texture_t *a = drawn.a;
	gs_texture_t *b = drawn.b;
	render2(tr, true);
	check_renders(2, 2, 0);
	assert_int_equal(drawn.calls, 2);
	assert_ptr_equal(drawn.a, a);
	assert_ptr_equal(drawn.b, b);
	check_slots(tr, live.child[0], live.child[1], true);
	assert_int_equal(video_stop_signals, 0);
}

/* Transitioning without a callback: slot B (the audio transitions too). */
static void transitioning_without_callback_draws_b(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("no callback");
	start_halfway(tr);

	render2(tr, false);
	check_renders(0, 1, 0);
	render2(tr, false);
	check_renders(0, 2, 0);
}

/* With the texture lock busy (the callback renders the same transition
 * again: its trylock fails), the nested render draws slot B and calls no
 * callback; the next render takes the callback again. */
static void busy_textures_draw_b(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("busy");
	start_halfway(tr);

	drawn.nest = true;
	render2(tr, true);
	assert_int_equal(drawn.calls, 1);
	check_renders(1, 2, 0);

	render2(tr, true);
	assert_int_equal(drawn.calls, 2);
	check_renders(2, 3, 0);
}

/* An empty slot gets the placeholder: from an empty slot A, and with both
 * slots empty, where nothing renders. */
static void empty_slot_gets_the_placeholder(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("placeholder");
	obs_transition_set(tr, NULL);
	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, live.child[1]));
	obs_transition_set_manual_time(tr, 0.5f);
	assert_true(time_is(tr, 0.5f));
	check_slots(tr, NULL, live.child[1], true);

	render2(tr, true);
	assert_int_equal(drawn.calls, 1);
	check_renders(0, 1, 0);
	assert_ptr_equal(drawn.a, live.placeholder);
	assert_non_null(drawn.b);
	assert_ptr_not_equal(drawn.b, live.placeholder);
}

/* The textures follow the mixed color space: sRGB is RGBA, 16-bit and
 * extended RGBA16F, re-created when the space changes. */
static void texture_format_follows_the_space(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("format");
	start_halfway(tr);

	render2(tr, true);
	assert_int_equal(drawn.a_format, GS_RGBA);
	assert_int_equal(drawn.b_format, GS_RGBA);

	recs[1].space = GS_CS_SRGB_16F;
	render2(tr, true);
	assert_int_equal(drawn.a_format, GS_RGBA16F);
	assert_int_equal(drawn.b_format, GS_RGBA16F);

	recs[1].space = GS_CS_SRGB;
	recs[0].space = GS_CS_709_SCRGB;
	render2(tr, true);
	assert_int_equal(drawn.a_format, GS_RGBA16F);
	assert_int_equal(drawn.b_format, GS_RGBA16F);

	recs[0].space = GS_CS_SRGB;
	render2(tr, true);
	assert_int_equal(drawn.a_format, GS_RGBA);
	assert_int_equal(drawn.b_format, GS_RGBA);
	assert_int_equal(drawn.calls, 4);
	check_renders(4, 4, 0);
}

/* At time 1.0 render2 ends the video and signals the video stop, once. It
 * counts the transition as active, so the audio runs on and nothing stops:
 * the slots stay, and it draws slot B. */
static void render2_ends_the_video(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("render2 stop");
	start_halfway(tr);
	obs_transition_set_manual_time(tr, 1.0f);
	assert_true(time_is(tr, 1.0f));

	render2(tr, true);
	assert_int_equal(drawn.calls, 0);
	check_renders(0, 1, 0);
	assert_int_equal(video_stop_signals, 1);
	assert_int_equal(stop_signals, 0);
	check_slots(tr, live.child[0], live.child[1], true);

	render2(tr, true);
	check_renders(0, 2, 0);
	assert_int_equal(video_stop_signals, 1);
	assert_int_equal(stop_signals, 0);
	check_slots(tr, live.child[0], live.child[1], true);
}

/* The direct render: slot A for either target while idle, the target's slot
 * while transitioning, and whether it transitions. */
static void direct_draws_the_target(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("direct");

	assert_false(render_direct(tr, OBS_TRANSITION_SOURCE_A));
	check_renders(1, 0, 0);
	assert_false(render_direct(tr, OBS_TRANSITION_SOURCE_B));
	check_renders(2, 0, 0);

	start_halfway(tr);
	assert_true(render_direct(tr, OBS_TRANSITION_SOURCE_A));
	check_renders(3, 0, 0);
	assert_true(render_direct(tr, OBS_TRANSITION_SOURCE_B));
	check_renders(3, 1, 0);
	assert_int_equal(video_stop_signals, 0);
	assert_int_equal(stop_signals, 0);
}

/* At time 1.0 on a transition that is not active, the direct render ends
 * the video and the audio and stops the transition: slot B becomes slot A,
 * the video stop and stop signals, and it draws the new slot A. Rendered
 * again, nothing more stops. */
static void direct_stops_an_inactive_transition(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("direct stop");
	start_halfway(tr);
	obs_transition_set_manual_time(tr, 1.0f);
	assert_true(time_is(tr, 1.0f));
	assert_false(obs_source_active(tr));

	assert_false(render_direct(tr, OBS_TRANSITION_SOURCE_B));
	check_renders(0, 1, 0);
	assert_int_equal(video_stop_signals, 1);
	assert_int_equal(stop_signals, 1);
	check_slots(tr, live.child[1], NULL, false);

	assert_false(render_direct(tr, OBS_TRANSITION_SOURCE_B));
	check_renders(0, 2, 0);
	assert_int_equal(video_stop_signals, 1);
	assert_int_equal(stop_signals, 1);
}

static const enum gs_color_space spaces[] = {
	GS_CS_SRGB,
	GS_CS_SRGB_16F,
	GS_CS_709_EXTENDED,
	GS_CS_709_SCRGB,
};

/* The mix: extended if either is extended or scRGB, else 16-bit if either
 * is, else sRGB. */
static enum gs_color_space mixed(enum gs_color_space a, enum gs_color_space b)
{
	if (a == GS_CS_709_EXTENDED || a == GS_CS_709_SCRGB || b == GS_CS_709_EXTENDED || b == GS_CS_709_SCRGB)
		return GS_CS_709_EXTENDED;
	if (a == GS_CS_SRGB_16F || b == GS_CS_SRGB_16F)
		return GS_CS_SRGB_16F;
	return GS_CS_SRGB;
}

static void check_asked(const struct child_rec *rec)
{
	assert_int_equal(rec->asked_count, 3);
	assert_int_equal(rec->asked[0], GS_CS_SRGB);
	assert_int_equal(rec->asked[1], GS_CS_SRGB_16F);
	assert_int_equal(rec->asked[2], GS_CS_709_EXTENDED);
}

/* sRGB with both slots empty; idle, slot A's space mixed with sRGB;
 * transitioning, both mixed, for every pair. */
static void color_space_mixes_the_slots(void **state)
{
	UNUSED_PARAMETER(state);
	skip_without_core();
	obs_source_t *tr = make_transition("space");

	for (size_t i = 0; i < OBS_COUNTOF(spaces); i++) {
		recs[0].space = spaces[i];
		recs[0].asked_count = 0;
		assert_int_equal(obs_transition_video_get_color_space(tr), mixed(spaces[i], GS_CS_SRGB));
		check_asked(&recs[0]);
	}

	assert_true(obs_transition_start(tr, OBS_TRANSITION_MODE_MANUAL, 0, live.child[1]));
	for (size_t i = 0; i < OBS_COUNTOF(spaces); i++) {
		for (size_t j = 0; j < OBS_COUNTOF(spaces); j++) {
			recs[0].space = spaces[i];
			recs[1].space = spaces[j];
			recs[1].asked_count = 0;
			enum gs_color_space got = obs_transition_video_get_color_space(tr);
			if (got != mixed(spaces[i], spaces[j])) {
				fprintf(stderr, "spaces %d and %d mixed to %d\n", (int)spaces[i], (int)spaces[j],
					(int)got);
				fail();
			}
			check_asked(&recs[1]);
		}
	}

	obs_transition_clear(tr);
	recs[0].asked_count = 0;
	assert_int_equal(obs_transition_video_get_color_space(tr), GS_CS_SRGB);
	assert_int_equal(recs[0].asked_count, 0);
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
		cmocka_unit_test_setup_teardown(idle_draws_a, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(transitioning_calls_back, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(transitioning_without_callback_draws_b, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(busy_textures_draw_b, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(empty_slot_gets_the_placeholder, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(texture_format_follows_the_space, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(render2_ends_the_video, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(direct_draws_the_target, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(direct_stops_an_inactive_transition, case_setup, case_teardown),
		cmocka_unit_test_setup_teardown(color_space_mixes_the_slots, case_setup, case_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
