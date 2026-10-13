#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include "scene-list-remove.h"

/* Upstream obsproject/obs-websocket#979 (reported by ximex, also AlbertSY):
 * deleting a scene in the Scenes dock sends SceneListChanged twice, while
 * adding one, or removing one through obs-websocket, sends it once.
 *
 * The model below stands in for OBSBasic and libobs:
 * - remove_source() is obs_source_remove(): libobs signals "source_remove",
 *   and OBSBasic::SourceRemoved runs OBSBasic::RemoveScene on the UI thread
 *   before obs_source_remove() returns, which is
 *   scene_list_on_scene_removed();
 * - take_from_list() takes the scene's item out of the scene list;
 * - scene_list_changed() emits OBS_FRONTEND_EVENT_SCENE_LIST_CHANGED and
 *   records how many scenes the list holds at that moment, which is what
 *   obs-websocket's SceneListChanged reports. */

#define MAX_SCENES 4
#define MAX_EVENTS 8

static const char scene_1[] = "Scene";
static const char scene_2[] = "Second";
static const char private_scene[] = "private";

struct frontend_model {
	const char *list[MAX_SCENES];
	int list_count;
	int events;
	int event_list_count[MAX_EVENTS];
	struct scene_list_remove_ops ops;
};

static bool model_take_from_list(void *param, void *scene)
{
	struct frontend_model *m = param;
	for (int i = 0; i < m->list_count; i++) {
		if (m->list[i] == scene) {
			memmove(&m->list[i], &m->list[i + 1], sizeof(m->list[0]) * (size_t)(m->list_count - i - 1));
			m->list_count--;
			return true;
		}
	}
	return false;
}

static void model_remove_source(void *param, void *scene)
{
	struct frontend_model *m = param;
	/* "source_remove" -> OBSBasic::SourceRemoved -> OBSBasic::RemoveScene */
	scene_list_on_scene_removed(&m->ops, scene);
}

static void model_scene_list_changed(void *param)
{
	struct frontend_model *m = param;
	if (m->events < MAX_EVENTS) {
		m->event_list_count[m->events] = m->list_count;
	}
	m->events++;
}

static void model_init(struct frontend_model *m)
{
	memset(m, 0, sizeof(*m));
	m->list[m->list_count++] = scene_1;
	m->list[m->list_count++] = scene_2;
	m->ops.param = m;
	m->ops.remove_source = model_remove_source;
	m->ops.take_from_list = model_take_from_list;
	m->ops.scene_list_changed = model_scene_list_changed;
}

/* Scenes dock: select "Second", Remove, Yes. One scene left, one event. */
static void test_remove_selected_announces_once(void **state)
{
	(void)state;
	struct frontend_model m;

	model_init(&m);
	scene_list_remove_selected(&m.ops, (void *)scene_2);

	printf("remove selected: %d SCENE_LIST_CHANGED event(s), expected 1\n", m.events);
	assert_int_equal(m.list_count, 1);
	assert_int_equal(m.events, 1);
}

/* Every event reports a list that differs from the one before it. */
static void test_no_event_without_list_change(void **state)
{
	(void)state;
	struct frontend_model m;

	model_init(&m);
	int before = m.list_count;
	scene_list_remove_selected(&m.ops, (void *)scene_2);

	int unchanged = 0;
	for (int i = 0; i < m.events && i < MAX_EVENTS; i++) {
		if (m.event_list_count[i] == before) {
			fprintf(stderr, "event %d reports %d scenes, unchanged\n", i + 1, before);
			unchanged++;
		}
		before = m.event_list_count[i];
	}
	assert_int_equal(unchanged, 0);
}

/* Guard: obs-websocket RemoveScene, a script, or redo of a scene removal go
 * through obs_source_remove() only, and are announced once, with the scene
 * already gone from the list. */
static void test_removed_elsewhere_announces_once(void **state)
{
	(void)state;
	struct frontend_model m;

	model_init(&m);
	model_remove_source(&m, (void *)scene_2);

	assert_int_equal(m.events, 1);
	assert_int_equal(m.event_list_count[0], 1);
}

/* Guard: removing a scene the list does not show (a private scene) is not a
 * scene list change. */
static void test_unlisted_scene_not_announced(void **state)
{
	(void)state;
	struct frontend_model m;

	model_init(&m);
	model_remove_source(&m, (void *)private_scene);

	assert_int_equal(m.list_count, 2);
	assert_int_equal(m.events, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_remove_selected_announces_once),
		cmocka_unit_test(test_no_event_without_list_change),
		cmocka_unit_test(test_removed_elsewhere_announces_once),
		cmocka_unit_test(test_unlisted_scene_not_announced),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
