/* obs_transition_set() must refuse a source that contains the transition.
 *
 * obs_transition_load() refuses a source that contains the transition (a
 * cycle) with "infinite recursion", and obs_transition_start() refuses such
 * a destination before it changes anything, but obs_transition_set() stored
 * it in slot A. While the slot holds it, the source's tree goes through the
 * transition back to the source, so any walk of its active tree (showing or
 * activating the source, the audio thread's walk of an output channel) locks
 * the transition's non-recursive mutex a second time and never returns.
 *
 * The cases build a scene holding a leaf and the transition (and an outer
 * scene holding that scene), set the transition to an unrelated source, show
 * the scene, set the transition to the scene, to the outer scene or to
 * itself, and check that the set is refused (slot A and the counts are as
 * before) and that showing the scene again returns. That walk runs under a
 * watchdog thread: if it has not returned after 5 seconds, the test prints
 * what hung and exits with a failure, so it fails instead of hanging. Control:
 * an acyclic source is still stored and held. Core video is needed, so the
 * test only builds on Linux and runs under xvfb-run; without a display the
 * cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <errno.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>

#include <obs-internal.h>
#include <util/platform.h>
#include <util/threading.h>

#define LEAF_ID "test_transition_set_cycle_leaf"
#define TRANSITION_ID "test_transition_set_cycle_transition"
#define WATCHDOG_MS 5000

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
	return "transition set cycle fixture";
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
	obs_scene_t *outer; /* holds the cycle scene, so its tree holds the transition */
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
	fx->outer = obs_scene_create_private("outer scene");
	if (!fx->scene || !fx->leaf || !fx->other || !fx->transition || !fx->outer)
		return -1;
	if (obs_source_get_type(fx->transition) != OBS_SOURCE_TYPE_TRANSITION || !fx->leaf->context.data) {
		fprintf(stderr, "fixture sources were not registered\n");
		return -1;
	}
	if (!obs_scene_add(fx->scene, fx->leaf) || !obs_scene_add(fx->scene, fx->transition) ||
	    !obs_scene_add(fx->outer, obs_scene_get_source(fx->scene)))
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
	obs_scene_release(fx->outer);
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

/* Exits the process if `what` does not finish in time: a hang must fail
 * the test, not stall it (the stuck thread holds the transition's mutex,
 * so nothing after it could run either). */
struct watchdog {
	os_event_t *done;
	pthread_t thread;
	const char *what;
};

static void *watchdog_thread(void *data)
{
	struct watchdog *dog = data;
	if (os_event_timedwait(dog->done, WATCHDOG_MS) == ETIMEDOUT) {
		fprintf(stderr, "HANG: showing the scene %s did not return within %d ms\n", dog->what, WATCHDOG_MS);
		fflush(stderr);
		_Exit(EXIT_FAILURE);
	}
	return NULL;
}

static void watchdog_arm(struct watchdog *dog, const char *what)
{
	dog->what = what;
	assert_int_equal(os_event_init(&dog->done, OS_EVENT_TYPE_MANUAL), 0);
	assert_int_equal(pthread_create(&dog->thread, NULL, watchdog_thread, dog), 0);
}

static void watchdog_disarm(struct watchdog *dog)
{
	os_event_signal(dog->done);
	pthread_join(dog->thread, NULL);
	os_event_destroy(dog->done);
}

/* Shows the scene again and hides it: walks its active tree twice. */
static void walk_scene(obs_source_t *scene, const char *what)
{
	struct watchdog dog;
	watchdog_arm(&dog, what);
	obs_source_inc_showing(scene);
	obs_source_dec_showing(scene);
	watchdog_disarm(&dog);
}

static bool slot_a_is(struct fixture *fx, obs_source_t *want, const char *when)
{
	obs_source_t *got = obs_transition_get_source(fx->transition, OBS_TRANSITION_SOURCE_A);
	obs_source_release(got);
	if (got == want)
		return true;
	fprintf(stderr, "%s: slot A holds '%s', expected '%s'\n", when, got ? obs_source_get_name(got) : "(none)",
		want ? obs_source_get_name(want) : "(none)");
	return false;
}

/* Sets the transition to `cycle` while the scene is the program of output
 * channel 0 and the transition holds fx->other; the set must be refused. */
static void check_refused(struct fixture *fx, obs_source_t *cycle, const char *what)
{
	obs_source_t *scene = scene_source(fx);
	bool ok = true;

	obs_transition_set(fx->transition, fx->other);
	obs_set_output_source(0, scene);
	wait_ticks();
	ok &= counts_are(scene, 1, 1, "shown");
	ok &= counts_are(fx->transition, 1, 1, "shown");
	ok &= counts_are(fx->other, 1, 1, "shown");

	obs_transition_set(fx->transition, cycle);
	ok &= slot_a_is(fx, fx->other, what);
	walk_scene(scene, what);
	wait_ticks();
	ok &= counts_are(scene, 1, 1, what);
	ok &= counts_are(fx->leaf, 1, 1, what);
	ok &= counts_are(fx->transition, 1, 1, what);
	ok &= counts_are(fx->other, 1, 1, what);
	ok &= leaf_callbacks_are(fx->leaf, 1, 0, 1, 0, what);

	obs_transition_set(fx->transition, NULL);
	obs_set_output_source(0, NULL);
	wait_ticks();
	ok &= counts_are(scene, 0, 0, "after hiding the scene");
	ok &= counts_are(fx->other, 0, 0, "after hiding the scene");
	ok &= leaf_callbacks_are(fx->leaf, 1, 1, 1, 1, "after hiding the scene");
	assert_true(ok);
}

/* The scene holds the transition as an item. */
static void set_containing_scene_is_refused(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	check_refused(fx, scene_source(fx), "after setting the scene that holds the transition");
}

/* The outer scene holds the scene that holds the transition. */
static void set_outer_scene_is_refused(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	check_refused(fx, obs_scene_get_source(fx->outer), "after setting the outer scene");
}

/* The transition itself. */
static void set_itself_is_refused(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	check_refused(fx, fx->transition, "after setting the transition to itself");
}

/* Control: a source outside the cycle is still stored and held. */
static void set_acyclic_source_is_stored(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	obs_source_t *scene = scene_source(fx);
	bool ok = true;

	obs_set_output_source(0, scene);
	obs_transition_set(fx->transition, fx->other);
	wait_ticks();
	ok &= slot_a_is(fx, fx->other, "after setting the source");
	ok &= counts_are(fx->other, 1, 1, "after setting the source");
	ok &= leaf_callbacks_are(fx->other, 1, 0, 1, 0, "after setting the source");
	walk_scene(scene, "with an acyclic source");

	obs_transition_set(fx->transition, NULL);
	wait_ticks();
	ok &= slot_a_is(fx, NULL, "after clearing the source");
	ok &= counts_are(fx->other, 0, 0, "after clearing the source");
	ok &= counts_are(scene, 1, 1, "after clearing the source");
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
		cmocka_unit_test_setup_teardown(set_acyclic_source_is_stored, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(set_containing_scene_is_refused, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(set_outer_scene_is_refused, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(set_itself_is_refused, fixture_setup, fixture_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
