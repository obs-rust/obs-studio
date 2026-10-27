/* obs_transition_load() must take back the source already in slot A.
 *
 * obs_load_sources() creates every source first, then loads each transition
 * and calls the callback for each source in order. A source set into a
 * transition before that transition is loaded (by the callback for an
 * earlier source, or by a plugin) is in slot A when the load runs. The load
 * wrote the loaded source over it without taking it back: the transition's
 * reference on the old source was lost, so the source was never destroyed,
 * and, while the transition was shown, the source stayed an active child of
 * it, still active and showing after the transition moved on.
 *
 * The cases load a leaf and a transition with obs_load_sources(). The
 * callback for the leaf sets the transition to a test source and, in the
 * shown cases, makes the transition the program of output channel 0 first.
 * The transition's saved slot A names another source (or the same one).
 * After the load the cases check slot A, the raw activate/show counts, the
 * leaf callbacks, and that the old source is destroyed once the test lets go
 * of it. Controls: a load onto an empty slot, and a load of the source
 * already in the slot. Core video is needed, so the test only builds on
 * Linux and runs under xvfb-run; without a display the cases skip.
 */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs-internal.h>
#include <util/platform.h>

#define LEAF_ID "test_transition_load_release_leaf"
#define TRANSITION_ID "test_transition_load_release_transition"
#define ANCHOR_NAME "load release anchor"
#define TRANSITION_NAME "load release transition"
#define OLD_NAME "load release old"
#define NEW_NAME "load release new"

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
	return "transition load release fixture";
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

/* Draws nothing: rendering is not what is tested. */
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
	obs_source_t *old;    /* set into slot A before the load */
	obs_source_t *loaded; /* named by the transition's saved slot A */
	obs_source_t *preset; /* what the callback sets, or NULL */
	bool show;            /* the callback shows the transition first */
	bool preset_shown;    /* the preset's callbacks ran before the load */
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
	fx->old = obs_source_create(LEAF_ID, OLD_NAME, NULL, NULL);
	fx->loaded = obs_source_create(LEAF_ID, NEW_NAME, NULL, NULL);
	if (!fx->old || !fx->loaded)
		return -1;
	if (!fx->old->context.data) {
		fprintf(stderr, "fixture sources were not registered\n");
		return -1;
	}
	return 0;
}

static int fixture_teardown(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		return 0;
	obs_set_output_source(0, NULL);
	if (fx->transition) {
		obs_transition_clear(fx->transition);
		obs_source_remove(fx->transition);
		obs_source_release(fx->transition);
	}
	if (fx->loaded) {
		obs_source_remove(fx->loaded);
		obs_source_release(fx->loaded);
	}
	if (fx->old) {
		obs_source_remove(fx->old);
		obs_source_release(fx->old);
	}
	obs_wait_for_destroy_queue();
	bfree(fx);
	return 0;
}

/* The activate/deactivate/show/hide callbacks run on the video tick, so
 * the checks wait for them, for at most this long. */
#define CALLBACK_TIMEOUT_MS 5000

static bool leaf_callbacks_reach(obs_source_t *source, long activates, long deactivates, long shows, long hides)
{
	struct leaf *leaf = source->context.data;
	for (int waited = 0;; waited += 10) {
		if (os_atomic_load_long(&leaf->activates) == activates &&
		    os_atomic_load_long(&leaf->deactivates) == deactivates &&
		    os_atomic_load_long(&leaf->shows) == shows && os_atomic_load_long(&leaf->hides) == hides)
			return true;
		if (waited >= CALLBACK_TIMEOUT_MS)
			return false;
		os_sleep_ms(10);
	}
}

/* Runs for each loaded source after its load. The anchor comes before the
 * transition, so this runs before the transition is loaded. */
static void load_cb(void *private_data, obs_source_t *source)
{
	struct fixture *fx = private_data;
	if (strcmp(obs_source_get_name(source), ANCHOR_NAME) != 0)
		return;

	/* Held by the fixture, so it outlives obs_load_sources(). */
	fx->transition = obs_get_source_by_name(TRANSITION_NAME);
	if (!fx->transition)
		return;
	if (fx->show)
		obs_set_output_source(0, fx->transition);
	if (fx->preset)
		obs_transition_set(fx->transition, fx->preset);
	/* Lets the preset source's activate and show callbacks run first, so
	 * the load takes back a source that was really shown. */
	if (fx->show && fx->preset)
		fx->preset_shown = leaf_callbacks_reach(fx->preset, 1, 0, 1, 0);
}

static obs_data_t *source_data(const char *id, const char *name)
{
	obs_data_t *data = obs_data_create();
	obs_data_set_string(data, "id", id);
	obs_data_set_string(data, "name", name);
	return data;
}

/* Loads the anchor and the transition, its saved slot A naming `slot_a`. */
static void load(struct fixture *fx, const char *slot_a)
{
	obs_data_array_t *array = obs_data_array_create();
	obs_data_t *anchor = source_data(LEAF_ID, ANCHOR_NAME);
	obs_data_t *transition = source_data(TRANSITION_ID, TRANSITION_NAME);
	obs_data_set_string(transition, "transition_source_a", slot_a);
	obs_data_array_push_back(array, anchor);
	obs_data_array_push_back(array, transition);
	obs_data_release(anchor);
	obs_data_release(transition);

	obs_load_sources(array, load_cb, fx);
	obs_data_array_release(array);

	assert_non_null(fx->transition);
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

static bool leaf_callbacks_are(obs_source_t *source, long activates, long deactivates, long shows, long hides,
			       const char *when)
{
	struct leaf *leaf = source->context.data;
	if (leaf_callbacks_reach(source, activates, deactivates, shows, hides))
		return true;
	long a = os_atomic_load_long(&leaf->activates);
	long d = os_atomic_load_long(&leaf->deactivates);
	long s = os_atomic_load_long(&leaf->shows);
	long h = os_atomic_load_long(&leaf->hides);
	fprintf(stderr, "%s: '%s' activate=%ld deactivate=%ld show=%ld hide=%ld, expected %ld/%ld/%ld/%ld\n", when,
		obs_source_get_name(source), a, d, s, h, activates, deactivates, shows, hides);
	return false;
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

static bool preset_was_shown(struct fixture *fx)
{
	if (fx->preset_shown)
		return true;
	fprintf(stderr, "before the load: '%s' was not activated and shown within %d ms\n",
		obs_source_get_name(fx->preset), CALLBACK_TIMEOUT_MS);
	return false;
}

/* Lets go of the test's own reference to `*source` and checks that nothing
 * else holds it: it is destroyed. */
static bool released_source_is_destroyed(obs_source_t **source, const char *when)
{
	obs_weak_source_t *weak = obs_source_get_weak_source(*source);
	const char *name = obs_source_get_name(*source);
	char copy[64];
	snprintf(copy, sizeof(copy), "%s", name);

	obs_source_remove(*source);
	obs_source_release(*source);
	*source = NULL;
	obs_wait_for_destroy_queue();

	bool expired = obs_weak_source_expired(weak);
	obs_weak_source_release(weak);
	if (expired)
		return true;
	fprintf(stderr, "%s: '%s' is still referenced after its last owner let go\n", when, copy);
	return false;
}

/* Slot A holds a source while the transition is shown; the load replaces
 * it. The old source leaves the transition: deactivated, hidden, and
 * destroyed once the test lets go of it. */
static void load_releases_shown_source(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	bool ok = true;

	fx->preset = fx->old;
	fx->show = true;
	load(fx, NEW_NAME);
	ok &= preset_was_shown(fx);
	ok &= slot_a_is(fx, fx->loaded, "after the load");
	ok &= counts_are(fx->loaded, 1, 1, "after the load");
	ok &= counts_are(fx->old, 0, 0, "after the load");
	ok &= leaf_callbacks_are(fx->old, 1, 1, 1, 1, "after the load");
	ok &= released_source_is_destroyed(&fx->old, "after the load");
	assert_true(ok);
}

/* As above, with the transition not shown: nothing is activated, and the
 * old source is still destroyed once the test lets go of it. */
static void load_releases_hidden_source(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	bool ok = true;

	fx->preset = fx->old;
	load(fx, NEW_NAME);
	ok &= slot_a_is(fx, fx->loaded, "after the load");
	ok &= counts_are(fx->loaded, 0, 0, "after the load");
	ok &= counts_are(fx->old, 0, 0, "after the load");
	ok &= released_source_is_destroyed(&fx->old, "after the load");
	assert_true(ok);
}

/* Slot A already holds the source the load names: it stays active and
 * showing once (not twice), and goes away with the transition. */
static void load_of_same_source_counts_once(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	bool ok = true;

	fx->preset = fx->loaded;
	fx->show = true;
	load(fx, NEW_NAME);
	ok &= preset_was_shown(fx);
	ok &= slot_a_is(fx, fx->loaded, "after the load");
	ok &= counts_are(fx->loaded, 1, 1, "after the load");

	obs_set_output_source(0, NULL);
	obs_transition_clear(fx->transition);
	ok &= counts_are(fx->loaded, 0, 0, "after clearing the transition");
	ok &= leaf_callbacks_are(fx->loaded, 1, 1, 1, 1, "after clearing the transition");
	ok &= released_source_is_destroyed(&fx->loaded, "after clearing the transition");
	assert_true(ok);
}

/* Control: a load onto an empty slot stores and shows the loaded source. */
static void load_onto_empty_slot(void **state)
{
	struct fixture *fx = *state;
	if (!fx)
		skip();
	bool ok = true;

	fx->show = true;
	load(fx, NEW_NAME);
	ok &= slot_a_is(fx, fx->loaded, "after the load");
	ok &= counts_are(fx->loaded, 1, 1, "after the load");
	ok &= counts_are(fx->old, 0, 0, "after the load");
	ok &= leaf_callbacks_are(fx->old, 0, 0, 0, 0, "after the load");
	ok &= released_source_is_destroyed(&fx->old, "after the load");
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
		cmocka_unit_test_setup_teardown(load_onto_empty_slot, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(load_releases_shown_source, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(load_releases_hidden_source, fixture_setup, fixture_teardown),
		cmocka_unit_test_setup_teardown(load_of_same_source_counts_once, fixture_setup, fixture_teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
