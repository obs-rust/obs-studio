/* Regression test for obsproject/obs-studio#4290 (reported by AskMP,
 * reproduced and traced by bershanskiy): with several sources selected in
 * the source list, Paste Filters pasted the copied filters into one of them
 * only. OBSBasic::on_actionPasteFilters_triggered() pasted into
 * GetCurrentSceneItem(), the current item, and ignored the rest of the
 * selection.
 *
 * The filters must go to the source of every selected item, including
 * selected items inside groups, once per source even when several selected
 * items use the same source (an item and a reference to it), never back
 * into the source they were copied from, and undo and redo must cover every
 * source pasted to.
 *
 * The test builds a scene with real libobs sources and a minimal registered
 * filter, selects items and pastes with the Qt-free helper the frontend
 * calls. It needs no video or graphics. obs_startup() needs a display on
 * Linux (run under xvfb-run); the cases skip when it cannot start. */

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <stdio.h>
#include <string.h>

#include <obs.h>
#include <util/base.h>
#include <util/dstr.h>

#include "filter-clipboard.h"

#define BOX_ID "filter_clipboard_box"
#define FILTER_ID "filter_clipboard_filter"

static bool obs_up;

/* Without video, destroying a source logs a debug line for every graphics
 * call it skips; keep warnings and errors only. */
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

static const char *filter_name(void *type_data)
{
	(void)type_data;
	return "test filter";
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
	struct obs_source_info filter = {
		.id = FILTER_ID,
		.type = OBS_SOURCE_TYPE_FILTER,
		.output_flags = OBS_SOURCE_VIDEO,
		.get_name = filter_name,
		.create = data_create,
		.destroy = data_destroy,
	};
	obs_register_source(&box);
	obs_register_source(&filter);
}

enum { SRC, B, C, D, NUM_SOURCES };

struct fixture {
	obs_scene_t *scene;
	obs_source_t *sources[NUM_SOURCES];
	obs_sceneitem_t *items[NUM_SOURCES];
};

static void add_filter(obs_source_t *source, const char *name)
{
	obs_source_t *filter = obs_source_create(FILTER_ID, name, NULL, NULL);
	assert_non_null(filter);
	obs_source_filter_add(source, filter);
	obs_source_release(filter);
}

/* A scene with the copy source SRC (two filters, "blur" and "crop") and
 * three more boxes, B, C and D, none selected. */
static int setup(void **state)
{
	static const char *names[NUM_SOURCES] = {"src", "b", "c", "d"};
	struct fixture *f;

	if (!obs_up)
		return 0;

	f = bzalloc(sizeof(*f));
	f->scene = obs_scene_create("filter clipboard scene");
	assert_non_null(f->scene);

	for (int i = 0; i < NUM_SOURCES; i++) {
		f->sources[i] = obs_source_create(BOX_ID, names[i], NULL, NULL);
		assert_non_null(f->sources[i]);
		f->items[i] = obs_scene_add(f->scene, f->sources[i]);
		assert_non_null(f->items[i]);
		obs_sceneitem_select(f->items[i], false);
	}

	add_filter(f->sources[SRC], "blur");
	add_filter(f->sources[SRC], "crop");

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

	obs_source_remove(obs_scene_get_source(f->scene));
	obs_scene_release(f->scene);
	for (int i = 0; i < NUM_SOURCES; i++) {
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

static void append_name(obs_source_t *parent, obs_source_t *child, void *param)
{
	struct dstr *names = param;
	(void)parent;
	if (names->len)
		dstr_cat(names, ",");
	dstr_cat(names, obs_source_get_name(child));
}

/* The source's filters, in order, as "name,name". */
static bool has_filters(obs_source_t *source, const char *expected)
{
	struct dstr names = {0};
	bool match;

	obs_source_enum_filters(source, append_name, &names);
	match = strcmp(names.array ? names.array : "", expected) == 0;
	if (!match)
		fprintf(stderr, "  '%s' has filters [%s], expected [%s]\n", obs_source_get_name(source),
			names.array ? names.array : "", expected);
	dstr_free(&names);
	return match;
}

static void check_filters(obs_source_t *source, const char *expected, bool *ok)
{
	if (!has_filters(source, expected))
		*ok = false;
}

static void check_count(size_t count, size_t expected, bool *ok)
{
	if (count != expected) {
		fprintf(stderr, "  pasted to %zu sources, expected %zu\n", count, expected);
		*ok = false;
	}
}

/* Control: one selected item gets the filters, the others nothing. */
static void test_paste_one_selected(void **state)
{
	struct fixture *f = *state;
	bool ok = true;

	if (!obs_up)
		skip();

	obs_sceneitem_select(f->items[B], true);
	check_count(filter_clipboard_paste(f->scene, f->sources[SRC], NULL, NULL), 1, &ok);

	check_filters(f->sources[B], "blur,crop", &ok);
	check_filters(f->sources[C], "", &ok);
	check_filters(f->sources[D], "", &ok);
	check_filters(f->sources[SRC], "blur,crop", &ok);
	if (!ok)
		fail();
}

/* Control: with nothing selected, nothing is pasted. */
static void test_paste_nothing_selected(void **state)
{
	struct fixture *f = *state;
	bool ok = true;

	if (!obs_up)
		skip();

	check_count(filter_clipboard_paste(f->scene, f->sources[SRC], NULL, NULL), 0, &ok);

	check_filters(f->sources[B], "", &ok);
	check_filters(f->sources[C], "", &ok);
	check_filters(f->sources[D], "", &ok);
	if (!ok)
		fail();
}

/* The report: several selected sources must all get the filters. */
static void test_paste_several_selected(void **state)
{
	struct fixture *f = *state;
	bool ok = true;

	if (!obs_up)
		skip();

	obs_sceneitem_select(f->items[B], true);
	obs_sceneitem_select(f->items[C], true);
	obs_sceneitem_select(f->items[D], true);
	check_count(filter_clipboard_paste(f->scene, f->sources[SRC], NULL, NULL), 3, &ok);

	check_filters(f->sources[B], "blur,crop", &ok);
	check_filters(f->sources[C], "blur,crop", &ok);
	check_filters(f->sources[D], "blur,crop", &ok);
	check_filters(f->sources[SRC], "blur,crop", &ok);
	if (!ok)
		fail();
}

/* bershanskiy's case from the upstream review: an item and a reference to
 * the same source, both selected, must paste into that source once, not
 * twice. */
static void test_paste_reference_once(void **state)
{
	struct fixture *f = *state;
	bool ok = true;

	if (!obs_up)
		skip();

	obs_sceneitem_t *ref = obs_scene_add(f->scene, f->sources[B]);
	assert_non_null(ref);

	obs_sceneitem_select(f->items[B], true);
	obs_sceneitem_select(ref, true);
	obs_sceneitem_select(f->items[C], true);
	check_count(filter_clipboard_paste(f->scene, f->sources[SRC], NULL, NULL), 2, &ok);

	check_filters(f->sources[B], "blur,crop", &ok);
	check_filters(f->sources[C], "blur,crop", &ok);
	check_filters(f->sources[D], "", &ok);
	if (!ok)
		fail();
}

/* The source the filters were copied from is in the selection: it keeps its
 * filters (no copies of its own filters), the others get them. */
static void test_paste_skips_copy_source(void **state)
{
	struct fixture *f = *state;
	bool ok = true;

	if (!obs_up)
		skip();

	obs_sceneitem_select(f->items[SRC], true);
	obs_sceneitem_select(f->items[B], true);
	obs_sceneitem_select(f->items[C], true);
	check_count(filter_clipboard_paste(f->scene, f->sources[SRC], NULL, NULL), 2, &ok);

	check_filters(f->sources[SRC], "blur,crop", &ok);
	check_filters(f->sources[B], "blur,crop", &ok);
	check_filters(f->sources[C], "blur,crop", &ok);
	if (!ok)
		fail();
}

/* A selected item inside a group gets the filters too, as with the other
 * multi-selection actions (Rotate, Paste Transform). The group itself is not
 * selected and gets none. */
static void test_paste_item_in_group(void **state)
{
	struct fixture *f = *state;
	bool ok = true;

	if (!obs_up)
		skip();

	obs_sceneitem_t *in_group[] = {f->items[C], f->items[D]};
	obs_sceneitem_t *group = obs_scene_insert_group(f->scene, "group", in_group, 2);
	assert_non_null(group);

	obs_sceneitem_select(group, false);
	obs_sceneitem_select(f->items[B], true);
	obs_sceneitem_select(f->items[D], true);
	check_count(filter_clipboard_paste(f->scene, f->sources[SRC], NULL, NULL), 2, &ok);

	check_filters(f->sources[B], "blur,crop", &ok);
	check_filters(f->sources[C], "", &ok);
	check_filters(f->sources[D], "blur,crop", &ok);
	check_filters(obs_sceneitem_get_source(group), "", &ok);
	if (!ok)
		fail();
}

/* One undo entry covers every source pasted to: undo puts back each
 * source's own filters, redo pastes again. */
static void test_paste_undo_redo(void **state)
{
	struct fixture *f = *state;
	bool ok = true;

	if (!obs_up)
		skip();

	add_filter(f->sources[B], "own");

	obs_sceneitem_select(f->items[B], true);
	obs_sceneitem_select(f->items[C], true);

	obs_data_t *undo = obs_data_create();
	obs_data_t *redo = obs_data_create();
	check_count(filter_clipboard_paste(f->scene, f->sources[SRC], undo, redo), 2, &ok);
	check_filters(f->sources[B], "own,blur,crop", &ok);
	check_filters(f->sources[C], "blur,crop", &ok);

	/* The frontend keeps undo and redo as JSON. */
	obs_data_t *undo_json = obs_data_create_from_json(obs_data_get_json(undo));
	obs_data_t *redo_json = obs_data_create_from_json(obs_data_get_json(redo));

	fprintf(stderr, "  undo:\n");
	filter_clipboard_restore(undo_json, NULL, NULL);
	check_filters(f->sources[B], "own", &ok);
	check_filters(f->sources[C], "", &ok);
	check_filters(f->sources[SRC], "blur,crop", &ok);

	fprintf(stderr, "  redo:\n");
	filter_clipboard_restore(redo_json, NULL, NULL);
	check_filters(f->sources[B], "own,blur,crop", &ok);
	check_filters(f->sources[C], "blur,crop", &ok);
	check_filters(f->sources[D], "", &ok);

	obs_data_release(undo_json);
	obs_data_release(redo_json);
	obs_data_release(undo);
	obs_data_release(redo);
	if (!ok)
		fail();
}

static int group_setup(void **state)
{
	(void)state;
	base_set_log_handler(log_handler, NULL);
	obs_up = obs_startup("en-US", NULL, NULL);
	if (obs_up)
		register_types();
	else
		fprintf(stderr, "obs_startup failed (no display?), skipping\n");
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
		cmocka_unit_test_setup_teardown(test_paste_one_selected, setup, teardown),
		cmocka_unit_test_setup_teardown(test_paste_nothing_selected, setup, teardown),
		cmocka_unit_test_setup_teardown(test_paste_several_selected, setup, teardown),
		cmocka_unit_test_setup_teardown(test_paste_reference_once, setup, teardown),
		cmocka_unit_test_setup_teardown(test_paste_skips_copy_source, setup, teardown),
		cmocka_unit_test_setup_teardown(test_paste_item_in_group, setup, teardown),
		cmocka_unit_test_setup_teardown(test_paste_undo_redo, setup, teardown),
	};

	return cmocka_run_group_tests(tests, group_setup, group_teardown);
}
