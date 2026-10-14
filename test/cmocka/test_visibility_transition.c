/* Regression test for obsproject/obs-studio#10448 (reported by BakaKuma-uwu):
 * with several sources selected in the Sources list, picking a Show
 * Transition or Hide Transition in the context menu gives the transition to
 * one of them only. OBSBasic::CreateVisibilityTransitionMenu() changes
 * GetCurrentSceneItem(), the current item, and ignores the rest of the
 * selection, although Paste in the same menu, Paste Filters and Paste
 * Transform act on every selected item.
 *
 * The transition, None and the duration must reach every selected item,
 * including selected items inside groups, and nothing else; an item that
 * already has a transition of the picked type keeps it; undo and redo must
 * cover every changed item.
 *
 * The test builds a scene with real libobs sources and a minimal registered
 * transition type, selects items and applies the menu choice with the Qt-free
 * helper the frontend calls. Transitions need graphics, so it loads the built
 * OpenGL module and runs under xvfb-run on Linux; without a display the cases
 * skip. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/bmem.h>

#include "visibility-transition.h"

#define BOX_ID "visibility_transition_box"
#define FADE_ID "visibility_transition_fade"
#define SLIDE_ID "visibility_transition_slide"
#define DEFAULT_MS 300

static bool obs_up;

static void log_handler(int lvl, const char *msg, va_list args, void *p)
{
	(void)p;
	if (lvl > LOG_WARNING)
		return;
	vfprintf(stderr, msg, args);
	fprintf(stderr, "\n");
}

static const char *box_name(void *type_data)
{
	(void)type_data;
	return "box";
}

static const char *fade_name(void *type_data)
{
	(void)type_data;
	return "test fade";
}

static const char *slide_name(void *type_data)
{
	(void)type_data;
	return "test slide";
}

static void *data_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void data_destroy(void *data)
{
	(void)data;
}

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

static uint32_t box_size(void *data)
{
	(void)data;
	return 64;
}

static void register_types(void)
{
	struct obs_source_info box = {
		.id = BOX_ID,
		.type = OBS_SOURCE_TYPE_INPUT,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = box_name,
		.create = data_create,
		.destroy = data_destroy,
		.get_width = box_size,
		.get_height = box_size,
	};
	struct obs_source_info fade = {
		.id = FADE_ID,
		.type = OBS_SOURCE_TYPE_TRANSITION,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = fade_name,
		.create = data_create,
		.destroy = data_destroy,
		.video_render = transition_video_render,
		.audio_render = transition_audio_render,
	};
	struct obs_source_info slide = fade;
	slide.id = SLIDE_ID;
	slide.get_name = slide_name;

	obs_register_source(&box);
	obs_register_source(&fade);
	obs_register_source(&slide);
}

/* A, B, C and D at the top of the scene, and a group holding E. */
enum { A, B, C, D, E, NUM_BOXES };

struct fixture {
	obs_scene_t *scene;
	obs_source_t *sources[NUM_BOXES];
	obs_sceneitem_t *items[NUM_BOXES];
	obs_sceneitem_t *group;
	obs_data_t *undo;
	obs_data_t *redo;
};

static int setup(void **state)
{
	static const char *names[NUM_BOXES] = {"a", "b", "c", "d", "e"};
	struct fixture *f;

	if (!obs_up)
		return 0;

	f = bzalloc(sizeof(*f));
	f->scene = obs_scene_create("visibility transition scene");
	assert_non_null(f->scene);

	for (int i = 0; i < NUM_BOXES; i++) {
		f->sources[i] = obs_source_create(BOX_ID, names[i], NULL, NULL);
		assert_non_null(f->sources[i]);
		f->items[i] = obs_scene_add(f->scene, f->sources[i]);
		assert_non_null(f->items[i]);
	}

	f->group = obs_scene_add_group(f->scene, "visibility transition group");
	assert_non_null(f->group);
	obs_sceneitem_group_add_item(f->group, f->items[E]);

	for (int i = 0; i < NUM_BOXES; i++)
		obs_sceneitem_select(f->items[i], false);
	obs_sceneitem_select(f->group, false);

	*state = f;
	return 0;
}

static void no_task(void *param)
{
	(void)param;
}

static int teardown(void **state)
{
	struct fixture *f = *state;

	if (!f)
		return 0;

	obs_data_release(f->undo);
	obs_data_release(f->redo);
	obs_source_remove(obs_scene_get_source(f->scene));
	obs_scene_release(f->scene);
	for (int i = 0; i < NUM_BOXES; i++) {
		obs_source_remove(f->sources[i]);
		obs_source_release(f->sources[i]);
	}
	/* Scenes are destroyed on libobs's destruction thread. Let that finish
	 * before the next case, and before obs_shutdown() frees what is left. */
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	bfree(f);
	*state = NULL;
	return 0;
}

/* Picks a transition in the menu opened for item. The undo and redo data
 * are kept in the fixture. Returns the number of items changed. */
static size_t pick(struct fixture *f, obs_sceneitem_t *item, bool show, const char *id)
{
	struct visibility_transition_change change = {
		.show = show,
		.id = id,
		.name_suffix = show ? "Show Transition" : "Hide Transition",
		.default_duration = DEFAULT_MS,
	};
	obs_data_release(f->undo);
	obs_data_release(f->redo);
	f->undo = obs_data_create();
	f->redo = obs_data_create();
	return visibility_transition_set(f->scene, item, &change, f->undo, f->redo);
}

static const char *transition_id(obs_sceneitem_t *item, bool show)
{
	obs_source_t *tr = obs_sceneitem_get_transition(item, show);
	return tr ? obs_source_get_id(tr) : "none";
}

/* Checks the show or hide transition type of every box against `expected`
 * (one entry per box, "none" for no transition). */
static void check_transitions(struct fixture *f, bool show, const char *const expected[NUM_BOXES])
{
	static const char *names = "ABCDE";
	bool ok = true;

	for (int i = 0; i < NUM_BOXES; i++) {
		const char *id = transition_id(f->items[i], show);
		if (strcmp(id, expected[i]) != 0) {
			fprintf(stderr, "  %c has %s transition %s, expected %s\n", names[i], show ? "show" : "hide",
				id, expected[i]);
			ok = false;
		}
	}
	assert_true(ok);
}

static void select_items(struct fixture *f, int count, const int *which)
{
	for (int i = 0; i < count; i++)
		obs_sceneitem_select(f->items[which[i]], true);
}

/* Controls: one selected item, as on master. */
static void show_transition_one_item(void **state)
{
	struct fixture *f = *state;
	if (!obs_up)
		skip();

	obs_sceneitem_select(f->items[B], true);
	assert_int_equal(pick(f, f->items[B], true, FADE_ID), 1);

	const char *const expected[NUM_BOXES] = {"none", FADE_ID, "none", "none", "none"};
	check_transitions(f, true, expected);
	assert_int_equal(obs_sceneitem_get_transition_duration(f->items[B], true), DEFAULT_MS);
	assert_string_equal(obs_source_get_name(obs_sceneitem_get_transition(f->items[B], true)), "b Show Transition");
}

static void same_type_keeps_the_transition(void **state)
{
	struct fixture *f = *state;
	if (!obs_up)
		skip();

	obs_sceneitem_select(f->items[A], true);
	pick(f, f->items[A], true, FADE_ID);
	obs_source_t *first = obs_sceneitem_get_transition(f->items[A], true);
	obs_sceneitem_set_transition_duration(f->items[A], true, 1000);

	assert_int_equal(pick(f, f->items[A], true, FADE_ID), 0);

	assert_ptr_equal(obs_sceneitem_get_transition(f->items[A], true), first);
	assert_int_equal(obs_sceneitem_get_transition_duration(f->items[A], true), 1000);
}

/* The bug: A, B and C selected, the menu opened for A. */
static void show_transition_three_items(void **state)
{
	struct fixture *f = *state;
	if (!obs_up)
		skip();

	const int sel[] = {A, B, C};
	select_items(f, 3, sel);
	size_t changed = pick(f, f->items[A], true, FADE_ID);

	const char *const expected[NUM_BOXES] = {FADE_ID, FADE_ID, FADE_ID, "none", "none"};
	check_transitions(f, true, expected);
	const char *const no_hide[NUM_BOXES] = {"none", "none", "none", "none", "none"};
	check_transitions(f, false, no_hide);
	assert_int_equal(obs_sceneitem_get_transition_duration(f->items[C], true), DEFAULT_MS);
	assert_string_equal(obs_source_get_name(obs_sceneitem_get_transition(f->items[C], true)), "c Show Transition");
	assert_int_equal(changed, 3);
}

static void hide_transition_three_items(void **state)
{
	struct fixture *f = *state;
	if (!obs_up)
		skip();

	const int sel[] = {B, C, D};
	select_items(f, 3, sel);
	pick(f, f->items[C], false, SLIDE_ID);

	const char *const expected[NUM_BOXES] = {"none", SLIDE_ID, SLIDE_ID, SLIDE_ID, "none"};
	check_transitions(f, false, expected);
}

static void selected_item_in_a_group(void **state)
{
	struct fixture *f = *state;
	if (!obs_up)
		skip();

	const int sel[] = {A, E};
	select_items(f, 2, sel);
	pick(f, f->items[A], true, SLIDE_ID);

	const char *const expected[NUM_BOXES] = {SLIDE_ID, "none", "none", "none", SLIDE_ID};
	check_transitions(f, true, expected);
}

static void other_type_and_none_for_three_items(void **state)
{
	struct fixture *f = *state;
	if (!obs_up)
		skip();

	/* Each of A, B and C picked on its own first. */
	for (int i = A; i <= C; i++) {
		obs_sceneitem_select(f->items[i], true);
		pick(f, f->items[i], true, FADE_ID);
		obs_sceneitem_select(f->items[i], false);
	}

	const int sel[] = {A, B, C};
	select_items(f, 3, sel);
	pick(f, f->items[A], true, SLIDE_ID);
	const char *const slide[NUM_BOXES] = {SLIDE_ID, SLIDE_ID, SLIDE_ID, "none", "none"};
	check_transitions(f, true, slide);

	pick(f, f->items[A], true, "");
	const char *const none[NUM_BOXES] = {"none", "none", "none", "none", "none"};
	check_transitions(f, true, none);
	assert_int_equal(obs_sceneitem_get_transition_duration(f->items[B], true), 0);
}

static void undo_and_redo_three_items(void **state)
{
	struct fixture *f = *state;
	if (!obs_up)
		skip();

	obs_sceneitem_select(f->items[B], true);
	pick(f, f->items[B], true, SLIDE_ID);
	obs_sceneitem_select(f->items[B], false);

	const int sel[] = {A, B, C};
	select_items(f, 3, sel);
	pick(f, f->items[A], true, FADE_ID);

	visibility_transition_restore(f->undo);
	const char *const before[NUM_BOXES] = {"none", SLIDE_ID, "none", "none", "none"};
	check_transitions(f, true, before);

	visibility_transition_restore(f->redo);
	const char *const after[NUM_BOXES] = {FADE_ID, FADE_ID, FADE_ID, "none", "none"};
	check_transitions(f, true, after);
}

static void duration_for_three_items(void **state)
{
	struct fixture *f = *state;
	if (!obs_up)
		skip();

	const int sel[] = {A, B, C};
	select_items(f, 3, sel);
	visibility_transition_set_duration(f->scene, f->items[A], false, 1200);

	assert_int_equal(obs_sceneitem_get_transition_duration(f->items[A], false), 1200);
	assert_int_equal(obs_sceneitem_get_transition_duration(f->items[B], false), 1200);
	assert_int_equal(obs_sceneitem_get_transition_duration(f->items[C], false), 1200);
	assert_int_equal(obs_sceneitem_get_transition_duration(f->items[D], false), 0);
	assert_int_equal(obs_sceneitem_get_transition_duration(f->items[A], true), 0);
}

static int group_setup(void **state)
{
	(void)state;
	base_set_log_handler(log_handler, NULL);

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

	register_types();
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
		cmocka_unit_test_setup_teardown(show_transition_one_item, setup, teardown),
		cmocka_unit_test_setup_teardown(same_type_keeps_the_transition, setup, teardown),
		cmocka_unit_test_setup_teardown(show_transition_three_items, setup, teardown),
		cmocka_unit_test_setup_teardown(hide_transition_three_items, setup, teardown),
		cmocka_unit_test_setup_teardown(selected_item_in_a_group, setup, teardown),
		cmocka_unit_test_setup_teardown(other_type_and_none_for_three_items, setup, teardown),
		cmocka_unit_test_setup_teardown(undo_and_redo_three_items, setup, teardown),
		cmocka_unit_test_setup_teardown(duration_for_three_items, setup, teardown),
	};
	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
