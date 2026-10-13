/*
 * Swapping the program transition in the middle of a transition must hand the
 * running transition over (upstream obsproject/obs-studio#11914).
 *
 * The frontend swaps the program transition with obs_transition_swap_begin()
 * and obs_transition_swap_end() for scene transition overrides (and when the
 * user picks another transition). Outside Studio Mode, with the default
 * transition Fade and a Cut override on scene A, switching A -> B -> A
 * quickly swaps Fade, half way to B, for Cut. Cut receives both scenes but is
 * not transitioning, so starting it towards A returns false ("A is already
 * shown"). The frontend then swaps Fade back in, again holding A and B
 * without transitioning, and every later start towards B finds B already set
 * as the destination and never transitions: the program stays on A until a
 * third scene is picked or Studio Mode is toggled.
 *
 * The test drives libobs the way OBSBasic::TransitionToScene,
 * OverrideTransition and TransitionFullyStopped do, with two transitions
 * that are not the program output (so nothing renders them and their state
 * stays where the calls leave it). It needs core video for the transitions'
 * render targets, so it runs under xvfb-run and skips without a display.
 */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdlib.h>

#include <obs.h>

static const char *tr_name(void *unused)
{
	UNUSED_PARAMETER(unused);
	return "test transition";
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

static struct obs_source_info fade_info = {
	.id = "test_swap_fade",
	.type = OBS_SOURCE_TYPE_TRANSITION,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = tr_name,
	.create = tr_create,
	.destroy = tr_destroy,
	.video_render = tr_video_render,
	.audio_render = tr_audio_render,
};

static struct obs_source_info cut_info = {
	.id = "test_swap_cut",
	.type = OBS_SOURCE_TYPE_TRANSITION,
	.output_flags = OBS_SOURCE_VIDEO,
	.get_name = tr_name,
	.create = tr_create,
	.destroy = tr_destroy,
	.video_render = tr_video_render,
	.audio_render = tr_audio_render,
};

static bool core_ready;

static bool init_core(void)
{
	if (!obs_startup("en-US", NULL, NULL))
		return false;

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
	if (obs_reset_video(&ovi) != OBS_VIDEO_SUCCESS)
		return false;

	obs_register_source(&fade_info);
	obs_register_source(&cut_info);
	return true;
}

static int setup(void **state)
{
	UNUSED_PARAMETER(state);
	const char *display = getenv("DISPLAY");
	if (display && *display)
		core_ready = init_core();
	return 0;
}

static int teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (obs_initialized())
		obs_shutdown();
	return 0;
}

struct fixture {
	obs_scene_t *a;
	obs_scene_t *b;
	obs_source_t *fade;
	obs_source_t *cut;
};

/* A copy of the running case's fixture, freed by case_teardown if a failed
 * assertion ended the case before it freed its fixture. */
static struct fixture live;
static bool live_set;

static void fixture_init(struct fixture *f)
{
	f->a = obs_scene_create_private("A");
	f->b = obs_scene_create_private("B");
	f->fade = obs_source_create_private("test_swap_fade", "Fade", NULL);
	f->cut = obs_source_create_private("test_swap_cut", "Cut", NULL);
	assert_non_null(f->a);
	assert_non_null(f->b);
	assert_non_null(f->fade);
	assert_non_null(f->cut);
	live = *f;
	live_set = true;
	assert_int_equal(obs_source_get_type(f->fade), OBS_SOURCE_TYPE_TRANSITION);
	assert_int_equal(obs_source_get_type(f->cut), OBS_SOURCE_TYPE_TRANSITION);
}

static void fixture_free(struct fixture *f)
{
	live_set = false;
	obs_transition_clear(f->fade);
	obs_transition_clear(f->cut);
	obs_source_release(f->fade);
	obs_source_release(f->cut);
	obs_scene_release(f->a);
	obs_scene_release(f->b);
}

static int case_teardown(void **state)
{
	UNUSED_PARAMETER(state);
	if (live_set)
		fixture_free(&live);
	return 0;
}

static obs_source_t *src(obs_scene_t *scene)
{
	return obs_scene_get_source(scene);
}

/* OBSBasic::OverrideTransition: make `to` the program transition. */
static void swap(obs_source_t *to, obs_source_t *from)
{
	obs_transition_swap_begin(to, from);
	obs_transition_swap_end(to, from);
}

static void assert_sources(obs_source_t *tr, obs_source_t *a, obs_source_t *b)
{
	obs_source_t *sa = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_A);
	obs_source_t *sb = obs_transition_get_source(tr, OBS_TRANSITION_SOURCE_B);
	assert_ptr_equal(sa, a);
	assert_ptr_equal(sb, b);
	obs_source_release(sa);
	obs_source_release(sb);
}

/* Program on A (Fade), switch to B: Fade runs from A to B. */
static void fade_a_to_b(struct fixture *f)
{
	obs_transition_set(f->fade, src(f->a));
	assert_true(obs_transition_start(f->fade, OBS_TRANSITION_MODE_AUTO, 1000, src(f->b)));
	assert_true(obs_transition_is_active(f->fade));
}

/* Swapping a running Fade for Cut hands the running transition to Cut. */
static void swap_hands_over_running_transition(void **state)
{
	UNUSED_PARAMETER(state);
	if (!core_ready)
		skip();

	struct fixture f;
	fixture_init(&f);

	fade_a_to_b(&f);
	swap(f.cut, f.fade);

	assert_sources(f.cut, src(f.a), src(f.b));
	assert_true(obs_transition_is_active(f.cut));

	fixture_free(&f);
}

/* Half way from A to B, switch back to A, whose override is Cut: Cut has to
 * start towards A. */
static void override_back_to_previous_scene_starts(void **state)
{
	UNUSED_PARAMETER(state);
	if (!core_ready)
		skip();

	struct fixture f;
	fixture_init(&f);

	fade_a_to_b(&f);
	swap(f.cut, f.fade);

	assert_true(obs_transition_start(f.cut, OBS_TRANSITION_MODE_AUTO, 0, src(f.a)));
	assert_true(obs_transition_is_active(f.cut));
	assert_sources(f.cut, src(f.b), src(f.a));

	fixture_free(&f);
}

/* The whole sequence from the report: after the override, Fade is swapped
 * back in (TransitionFullyStopped) and switching to B has to transition. */
static void switch_after_interrupted_override_transitions(void **state)
{
	UNUSED_PARAMETER(state);
	if (!core_ready)
		skip();

	struct fixture f;
	fixture_init(&f);

	fade_a_to_b(&f);
	swap(f.cut, f.fade);
	obs_transition_start(f.cut, OBS_TRANSITION_MODE_AUTO, 0, src(f.a));
	swap(f.fade, f.cut);

	assert_true(obs_transition_start(f.fade, OBS_TRANSITION_MODE_AUTO, 1000, src(f.b)));
	assert_true(obs_transition_is_active(f.fade));
	obs_source_t *dest = obs_transition_get_source(f.fade, OBS_TRANSITION_SOURCE_B);
	assert_ptr_equal(dest, src(f.b));
	obs_source_release(dest);

	fixture_free(&f);
}

/* Guard: swapping an idle transition hands over the scene shown, and the new
 * transition starts normally. */
static void swap_idle_transition(void **state)
{
	UNUSED_PARAMETER(state);
	if (!core_ready)
		skip();

	struct fixture f;
	fixture_init(&f);

	obs_transition_set(f.fade, src(f.a));
	swap(f.cut, f.fade);

	assert_false(obs_transition_is_active(f.cut));
	assert_sources(f.cut, src(f.a), NULL);
	assert_true(obs_transition_start(f.cut, OBS_TRANSITION_MODE_AUTO, 0, src(f.b)));
	assert_true(obs_transition_is_active(f.cut));

	fixture_free(&f);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_teardown(swap_hands_over_running_transition, case_teardown),
		cmocka_unit_test_teardown(override_back_to_previous_scene_starts, case_teardown),
		cmocka_unit_test_teardown(switch_after_interrupted_override_transitions, case_teardown),
		cmocka_unit_test_teardown(swap_idle_transition, case_teardown),
	};

	return cmocka_run_group_tests(tests, setup, teardown);
}
