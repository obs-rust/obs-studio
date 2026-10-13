#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include "studio-mode-exit.h"

/* Upstream obsproject/obs-websocket#1150 (reported by peddermaster2, also
 * WarmUpTill/SceneSwitcher#1326): turning Studio Mode off with scene A on the
 * program and scene B in the preview sends two CurrentProgramSceneChanged
 * events, B and then A, although the program shows A all along. Scene
 * switch triggers (Advanced Scene Switcher macros, websocket clients) fire
 * twice, the first time for a scene that never went live.
 *
 * The model below stands in for OBSBasic outside Studio Mode:
 * - select_scene() selects a scene in the scene list (OBSBasic::currentScene);
 * - show_on_program() puts a scene on the program output;
 * - scene_changed() emits OBS_FRONTEND_EVENT_SCENE_CHANGED. Listeners then ask
 *   obs_frontend_get_current_scene(), which outside Studio Mode returns the
 *   scene selected in the list (OBSStudioAPI.cpp), so each event is recorded
 *   with that scene. */

#define MAX_EVENTS 8

static const char scene_a[] = "A";
static const char scene_b[] = "B";

struct frontend_model {
	const char *list_scene;
	const char *program_scene;
	int events;
	const char *event_scene[MAX_EVENTS];
	const char *event_program[MAX_EVENTS];
};

static const char *name(const char *scene)
{
	return scene ? scene : "(none)";
}

static void model_select_scene(void *param, void *scene)
{
	struct frontend_model *m = param;
	m->list_scene = scene;
}

static void model_show_on_program(void *param, void *scene)
{
	struct frontend_model *m = param;
	m->program_scene = scene;
}

static void model_scene_changed(void *param)
{
	struct frontend_model *m = param;
	if (m->events < MAX_EVENTS) {
		m->event_scene[m->events] = m->list_scene;
		m->event_program[m->events] = m->program_scene;
	}
	m->events++;
}

static void exit_studio_mode(struct frontend_model *m, const char *program, const char *preview)
{
	struct studio_mode_exit_ops ops = {
		.param = m,
		.select_scene = model_select_scene,
		.show_on_program = model_show_on_program,
		.scene_changed = model_scene_changed,
	};

	memset(m, 0, sizeof(*m));
	m->list_scene = preview;
	m->program_scene = program;
	studio_mode_exit_switch(&ops, (void *)program, (void *)preview);
}

static void print_events(const struct frontend_model *m)
{
	fprintf(stderr, "  %d scene changed event(s):", m->events);
	for (int i = 0; i < m->events && i < MAX_EVENTS; i++)
		fprintf(stderr, " %s", name(m->event_scene[i]));
	fprintf(stderr, "\n");
}

/* `expected_events` scene changed events (0 or 1), each naming the scene on
 * the program, and the scene list and program both on `expected` afterwards. */
static void expect_switch(const struct frontend_model *m, int expected_events, const char *expected)
{
	bool ok = m->events == expected_events && m->list_scene == expected && m->program_scene == expected;

	for (int i = 0; ok && i < m->events && i < MAX_EVENTS; i++)
		ok = m->event_scene[i] == expected && m->event_program[i] == expected;

	if (!ok) {
		fprintf(stderr, "expected %d scene changed event(s)%s%s\n", expected_events,
			expected_events ? " naming " : "", expected_events ? name(expected) : "");
		print_events(m);
		for (int i = 0; i < m->events && i < MAX_EVENTS; i++) {
			if (m->event_scene[i] != m->event_program[i])
				fprintf(stderr, "  event %d names %s while the program shows %s\n", i + 1,
					name(m->event_scene[i]), name(m->event_program[i]));
		}
		fprintf(stderr, "  afterwards: scene list %s, program %s (expected %s)\n", name(m->list_scene),
			name(m->program_scene), name(expected));
		fail();
	}
}

/* The reported case: program A, preview B. The program keeps showing A, so
 * no scene changed event; the scene list moves to A. */
static void test_program_and_preview_differ(void **state)
{
	(void)state;
	struct frontend_model m;

	exit_studio_mode(&m, scene_a, scene_b);
	expect_switch(&m, 0, scene_a);
}

/* Program and preview on the same scene (adhawkins' and Zarithya's case:
 * Studio Mode toggled on and off): nothing changes, no event. */
static void test_program_and_preview_same(void **state)
{
	(void)state;
	struct frontend_model m;

	exit_studio_mode(&m, scene_a, scene_a);
	expect_switch(&m, 0, scene_a);
}

/* No event may name a scene the program does not show. */
static void test_no_event_names_preview(void **state)
{
	(void)state;
	struct frontend_model m;

	exit_studio_mode(&m, scene_a, scene_b);
	for (int i = 0; i < m.events && i < MAX_EVENTS; i++) {
		if (m.event_scene[i] != m.event_program[i]) {
			fprintf(stderr, "event %d names %s while the program shows %s\n", i + 1, name(m.event_scene[i]),
				name(m.event_program[i]));
			fail();
		}
	}
}

/* Guard: the program scene is gone (removed while in Studio Mode). The
 * preview scene goes to the program: that is a change, announced once. */
static void test_program_scene_gone(void **state)
{
	(void)state;
	struct frontend_model m;

	exit_studio_mode(&m, NULL, scene_b);
	expect_switch(&m, 1, scene_b);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_program_and_preview_differ),
		cmocka_unit_test(test_program_and_preview_same),
		cmocka_unit_test(test_no_event_names_preview),
		cmocka_unit_test(test_program_scene_gone),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
