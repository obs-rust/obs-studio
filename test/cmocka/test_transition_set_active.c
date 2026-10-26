/* obs_transition_set() must take back only the active reference it gave.
 *
 * obs_transition_set() puts the new source in slot A and marks the slot
 * active, then adds the source as an active child of the transition. The add
 * fails when the source contains the transition (a cycle), so the transition
 * holds no reference on it; the slot was still marked active. The next
 * obs_transition_set() then removes an active child that was never added,
 * and takes away a reference another parent holds: the source, and the
 * sources in it, are deactivated (or hidden) while that parent still shows
 * them.
 *
 * The cases build a scene holding a leaf and the transition, show the scene
 * through an output channel (main view) or as a projector would (auxiliary
 * view), set the transition to that scene and then to nothing, and check the
 * raw activate/show counts of the scene, the leaf and the transition, and
 * the leaf's deactivate/hide callbacks. Controls: an acyclic source is still
 * activated and released by the transition, and a cyclic source followed by
 * an acyclic one leaves both right. Core video is needed, so the test only
 * builds on Linux and runs under xvfb-run; without a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>

#include <obs-internal.h>
#include <util/platform.h>

#define LEAF_ID "test_transition_set_active_leaf"
#define TRANSITION_ID "test_transition_set_active_transition"

static bool obs_up;

struct leaf {
	long activates;
	long deactivates;
	long shows;
	long hides;
};

static const char *fixture_name(void *data)
{
	(void)data;
	return "transition set active fixture";
}

static void *leaf_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	(void)source;
	return bzalloc(sizeof(struct leaf));
}

static void leaf_destroy(void *data)
{
	bfree(data);
}

static uint32_t leaf_size(void *data)
{
	(void)data;
	return 16;
}

static void leaf_activate(void *data)
{
	os_atomic_inc_long(&((struct leaf *)data)->activates);
}

static void leaf_deactivate(void *data)
{
	os_atomic_inc_long(&((struct leaf *)data)->deactivates);
}

static void leaf_show(void *data)
{
	os_atomic_inc_long(&((struct leaf *)data)->shows);
}

static void leaf_hide(void *data)
{
	os_atomic_inc_long(&((struct leaf *)data)->hides);
}

static void *transition_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void transition_destroy(void *data)
{
	(void)data;
}

/* Draws nothing: rendering the cycle is not what is tested. */
static void transition_video_render(void *data, gs_effect_t *effect)
{
	(void)data;
	(void)effect;
}

static bool transition_audio_render(void *data, uint64_t *ts_out, struct obs_source_audio_mix *audio, uint32_t mixers,
				    size_t channels, size_t sample_rate)
{
	(void)data;
	(void)ts_out;
	(void)audio;
	(void)mixers;
	(void)channels;
	(void)sample_rate;
	return false;
}

static void register_sources(void)
{
	struct obs_source_info leaf = {
		.id = LEAF_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = fixture_name,
		.create = leaf_create,
		.destroy = leaf_destroy,
		.get_width = leaf_size,
		.get_height = leaf_size,
		.activate = leaf_activate,
		.deactivate = leaf_deactivate,
		.show = leaf_show,
		.hide = leaf_hide,
	};
	obs_register_source(&leaf);

	struct obs_source_info transition = {
		.id = TRANSITION_ID,
		.type = OBS_SOURCE_TYPE_TRANSITION,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = fixture_name,
		.create = transition_create,
		.destroy = transition_destroy,
		.video_render = transition_video_render,
		.audio_render = transition_audio_render,
	};
	obs_register_source(&transition);
}

struct fixture {
	obs_scene_t *scene;
	obs_source_t *leaf;
	obs_source_t *other;
	obs_source_t *transition;
};

static int fixture_setup(void **state)
{
	if (!obs_up) {
		*state = NULL;
		return 0;
	}

	struct fixture *fx = bzalloc(sizeof(*fx));
	*state = fx;
	fx->scene = obs_scene_create_private("cycle scene");
	fx->leaf = obs_source_create_private(LEAF_ID, "leaf", NULL);
	fx->other = obs_source_create_private(LEAF_ID, "other", NULL);
	fx->transition = obs_source_create_private(TRANSITION_ID, "transition", NULL);
	if (!fx->scene || !fx->leaf || !fx->other || !fx->transition)
		return -1;
	if (obs_source_get_type(fx->transition) != OBS_SOURCE_TYPE_TRANSITION || !fx->leaf->context.data) {
		fprintf(stderr, "fixture sources were not registered\n");
		return -1;
	}
	if (!obs_scene_add(fx->scene, fx->leaf) || !obs_scene_add(fx->scene, fx->transition))
		return -1;
	return 0;
}

static int fixture_teardown(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		return 0;
	obs_set_output_source(0, NULL);
	if (fx->transition)
		obs_transition_clear(fx->transition);
	obs_scene_release(fx->scene);
	obs_source_release(fx->transition);
	obs_source_release(fx->other);
	obs_source_release(fx->leaf);
	obs_wait_for_destroy_queue();
	bfree(fx);
	return 0;
}

static bool counts_are(obs_source_t *source, long want_active, long want_showing, const char *when)
{
	long active = os_atomic_load_long(&source->activate_refs);
	long showing = os_atomic_load_long(&source->show_refs);
	if (active == want_active && showing == want_showing)
		return true;
	fprintf(stderr, "%s: '%s' activate_refs=%ld show_refs=%ld, expected %ld/%ld\n", when,
		obs_source_get_name(source), active, showing, want_active, want_showing);
	return false;
}

/* The activate/deactivate/show/hide callbacks run on the video tick. */
static void wait_ticks(void)
{
	os_sleep_ms(300);
}

static bool leaf_callbacks_are(obs_source_t *source, long activates, long deactivates, long shows, long hides,
			       const char *when)
{
	struct leaf *leaf = source->context.data;
	long a = os_atomic_load_long(&leaf->activates);
	long d = os_atomic_load_long(&leaf->deactivates);
	long s = os_atomic_load_long(&leaf->shows);
	long h = os_atomic_load_long(&leaf->hides);
	if (a == activates && d == deactivates && s == shows && h == hides)
		return true;
	fprintf(stderr, "%s: '%s' activate=%ld deactivate=%ld show=%ld hide=%ld, expected %ld/%ld/%ld/%ld\n", when,
		obs_source_get_name(source), a, d, s, h, activates, deactivates, shows, hides);
	return false;
}

static obs_source_t *scene_source(struct fixture *fx)
{
	return obs_scene_get_source(fx->scene);
}

/* The scene is the program of output channel 0 (main view). */
static void cycle_keeps_output_channel_reference(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	obs_source_t *scene = scene_source(fx);
	bool ok = true;

	obs_set_output_source(0, scene);
	wait_ticks();
	ok &= counts_are(scene, 1, 1, "shown");
	ok &= counts_are(fx->leaf, 1, 1, "shown");
	ok &= counts_are(fx->transition, 1, 1, "shown");
	ok &= leaf_callbacks_are(fx->leaf, 1, 0, 1, 0, "shown");

	/* The scene contains the transition: it is not added as a child. */
	obs_transition_set(fx->transition, scene);
	ok &= counts_are(scene, 1, 1, "after setting the cycle");
	ok &= counts_are(fx->leaf, 1, 1, "after setting the cycle");

	obs_transition_set(fx->transition, NULL);
	wait_ticks();
	ok &= counts_are(scene, 1, 1, "after clearing the cycle");
	ok &= counts_are(fx->leaf, 1, 1, "after clearing the cycle");
	ok &= counts_are(fx->transition, 1, 1, "after clearing the cycle");
	ok &= leaf_callbacks_are(fx->leaf, 1, 0, 1, 0, "after clearing the cycle");

	obs_set_output_source(0, NULL);
	wait_ticks();
	ok &= counts_are(scene, 0, 0, "after hiding the scene");
	ok &= counts_are(fx->leaf, 0, 0, "after hiding the scene");
	ok &= counts_are(fx->transition, 0, 0, "after hiding the scene");
	ok &= leaf_callbacks_are(fx->leaf, 1, 1, 1, 1, "after hiding the scene");
	assert_true(ok);
}

/* The scene is shown only, as in a projector (auxiliary view). */
static void cycle_keeps_shown_reference(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	obs_source_t *scene = scene_source(fx);
	bool ok = true;

	obs_source_inc_showing(scene);
	wait_ticks();
	ok &= counts_are(scene, 0, 1, "shown");
	ok &= counts_are(fx->leaf, 0, 1, "shown");
	ok &= leaf_callbacks_are(fx->leaf, 0, 0, 1, 0, "shown");

	obs_transition_set(fx->transition, scene);
	obs_transition_set(fx->transition, NULL);
	wait_ticks();
	ok &= counts_are(scene, 0, 1, "after clearing the cycle");
	ok &= counts_are(fx->leaf, 0, 1, "after clearing the cycle");
	ok &= counts_are(fx->transition, 0, 1, "after clearing the cycle");
	ok &= leaf_callbacks_are(fx->leaf, 0, 0, 1, 0, "after clearing the cycle");

	obs_source_dec_showing(scene);
	wait_ticks();
	ok &= counts_are(scene, 0, 0, "after hiding the scene");
	ok &= counts_are(fx->leaf, 0, 0, "after hiding the scene");
	ok &= leaf_callbacks_are(fx->leaf, 0, 0, 1, 1, "after hiding the scene");
	assert_true(ok);
}

/* Control: a source outside the cycle is still held and released. */
static void acyclic_source_is_held_and_released(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	bool ok = true;

	obs_set_output_source(0, scene_source(fx));
	obs_transition_set(fx->transition, fx->other);
	wait_ticks();
	ok &= counts_are(fx->other, 1, 1, "after setting the source");
	ok &= leaf_callbacks_are(fx->other, 1, 0, 1, 0, "after setting the source");

	obs_transition_set(fx->transition, NULL);
	wait_ticks();
	ok &= counts_are(fx->other, 0, 0, "after clearing the source");
	ok &= counts_are(scene_source(fx), 1, 1, "after clearing the source");
	ok &= leaf_callbacks_are(fx->other, 1, 1, 1, 1, "after clearing the source");
	assert_true(ok);
}

/* A cyclic source replaced by an acyclic one: both counts stay right. */
static void cycle_then_acyclic_source(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	obs_source_t *scene = scene_source(fx);
	bool ok = true;

	obs_set_output_source(0, scene);
	obs_transition_set(fx->transition, scene);
	obs_transition_set(fx->transition, fx->other);
	wait_ticks();
	ok &= counts_are(scene, 1, 1, "after replacing the cycle");
	ok &= counts_are(fx->leaf, 1, 1, "after replacing the cycle");
	ok &= counts_are(fx->other, 1, 1, "after replacing the cycle");

	obs_transition_set(fx->transition, NULL);
	wait_ticks();
	ok &= counts_are(scene, 1, 1, "after clearing");
	ok &= counts_are(fx->other, 0, 0, "after clearing");
	ok &= leaf_callbacks_are(fx->leaf, 1, 0, 1, 0, "after clearing");
	assert_true(ok);
}

static int group_setup(void **state)
{
	(void)state;
	/* obs_startup() and core video need a display on Linux (run under
	 * xvfb-run); the cases skip without one. */
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

	register_sources();
	return 0;
}

static int group_teardown(void **state)
{
	(void)state;
	if (obs_up)
		obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(cycle_keeps_output_channel_reference, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(cycle_keeps_shown_reference, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(acyclic_source_is_held_and_released, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(cycle_then_acyclic_source, fixture_setup, fixture_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
