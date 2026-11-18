#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <cmocka.h>

#include "studio-mode-swap.h"

/* Upstream obsproject/obs-studio#14217 (reported by kdraslan), #5017 here:
 * in Studio Mode, a program switch requested while the program transition
 * still runs is refused (the program keeps showing the transition's target),
 * but OBSBasic::TransitionToScene() stored the refused scene as the program
 * anyway. obs_frontend_get_current_scene() and obs-websocket's
 * GetCurrentProgramScene then name the refused scene, and turning Studio
 * Mode off cuts the output to it.
 *
 * The model below stands in for OBSBasic in Studio Mode: request() is
 * TransitionToScene() (state is what OBSBasic stores), transition_end() is
 * the end of the program transition, when OBSBasic swaps the old program
 * into the preview (studio_mode_swap_scene()). Scenes are opaque pointers. */

static char starting[] = "Starting";
static char intro[] = "Intro";
static char ending[] = "Ending";
static char other[] = "Other";

struct studio_model {
	struct studio_mode_program state;
	char *preview;
	char *on_air;
	bool transitioning;
};

static void model_init(struct studio_model *m, char *program, char *preview)
{
	m->state.program = program;
	m->state.last_program = NULL;
	m->state.swap_scene = NULL;
	m->state.swap_preview = NULL;
	m->preview = preview;
	m->on_air = program;
	m->transitioning = false;
}

/* A double-click in the scene list or SetCurrentProgramScene: not forced, no
 * fade to black, no T-bar, "Duplicate Scene" off, swap on (the defaults). */
static enum studio_mode_program_step request(struct studio_model *m, char *target)
{
	enum studio_mode_program_step step =
		studio_mode_program_switch(&m->state, target, m->preview, false, true, false, true, m->transitioning);

	if (step == STUDIO_MODE_PROGRAM_SWITCH) {
		m->on_air = target;
		m->transitioning = true;
	}
	return step;
}

static void transition_end(struct studio_model *m)
{
	char *to = studio_mode_swap_scene(m->state.swap_scene, m->state.swap_preview, m->preview);

	if (to) {
		m->preview = to;
	}
	m->state.swap_scene = NULL;
	m->state.swap_preview = NULL;
	m->transitioning = false;
}

static const char *name(void *scene)
{
	return scene ? scene : "(none)";
}

/* The report: Starting on the program and in the preview, double-click
 * Intro, then Ending while the fade to Intro runs. */
static void test_refused_switch_keeps_program(void **state)
{
	(void)state;
	struct studio_model m;
	model_init(&m, starting, starting);

	assert_int_equal(request(&m, intro), STUDIO_MODE_PROGRAM_SWITCH);
	assert_int_equal(request(&m, ending), STUDIO_MODE_PROGRAM_REFUSED);

	printf("on air %s, stored program %s, last program %s\n", m.on_air, name(m.state.program),
	       name(m.state.last_program));

	assert_ptr_equal(m.on_air, intro);
	assert_ptr_equal(m.state.program, intro);
	assert_ptr_equal(m.state.last_program, starting);

	transition_end(&m);
	assert_ptr_equal(m.state.program, intro);
}

/* A refused switch does not change the swap the running transition
 * scheduled: program Starting, preview Other, switch to Intro (the old
 * program Starting goes to the preview at the end), then Ending is refused. */
static void test_refused_switch_keeps_swap(void **state)
{
	(void)state;
	struct studio_model m;
	model_init(&m, starting, other);

	assert_int_equal(request(&m, intro), STUDIO_MODE_PROGRAM_SWITCH);
	assert_ptr_equal(m.state.swap_scene, starting);
	assert_ptr_equal(m.state.swap_preview, other);

	assert_int_equal(request(&m, ending), STUDIO_MODE_PROGRAM_REFUSED);

	printf("swap scene %s, swap preview %s\n", name(m.state.swap_scene), name(m.state.swap_preview));

	assert_ptr_equal(m.state.swap_scene, starting);
	assert_ptr_equal(m.state.swap_preview, other);

	transition_end(&m);
	assert_ptr_equal(m.state.program, intro);
	assert_ptr_equal(m.preview, starting);
}

/* Guard: once the transition has ended, the same request goes through. */
static void test_switch_after_transition(void **state)
{
	(void)state;
	struct studio_model m;
	model_init(&m, starting, starting);

	assert_int_equal(request(&m, intro), STUDIO_MODE_PROGRAM_SWITCH);
	transition_end(&m);
	assert_int_equal(request(&m, ending), STUDIO_MODE_PROGRAM_SWITCH);

	assert_ptr_equal(m.on_air, ending);
	assert_ptr_equal(m.state.program, ending);
	assert_ptr_equal(m.state.last_program, intro);
}

/* Guard: a request for the program scene does nothing, during a transition
 * too. */
static void test_same_scene(void **state)
{
	(void)state;
	struct studio_model m;
	model_init(&m, intro, starting);

	assert_int_equal(request(&m, intro), STUDIO_MODE_PROGRAM_SAME);
	assert_ptr_equal(m.state.program, intro);
	assert_ptr_equal(m.state.last_program, intro);
	assert_null(m.state.swap_scene);

	m.transitioning = true;
	assert_int_equal(request(&m, intro), STUDIO_MODE_PROGRAM_SAME);
	assert_ptr_equal(m.state.program, intro);
}

/* Guard: with the T-bar held, the program before the T-bar stays the last
 * program. */
static void test_tbar_keeps_last_program(void **state)
{
	(void)state;
	struct studio_mode_program s = {intro, starting, NULL, NULL};

	assert_int_equal(studio_mode_program_switch(&s, ending, other, true, true, false, true, false),
			 STUDIO_MODE_PROGRAM_SWITCH);
	assert_ptr_equal(s.program, ending);
	assert_ptr_equal(s.last_program, starting);
	assert_ptr_equal(s.swap_scene, starting);
	assert_ptr_equal(s.swap_preview, other);
}

/* Guard: a forced switch or a fade to black neither stops at the program
 * scene nor schedules a swap. */
static void test_forced_switch(void **state)
{
	(void)state;
	struct studio_mode_program s = {intro, NULL, NULL, NULL};

	assert_int_equal(studio_mode_program_switch(&s, intro, other, false, false, false, true, false),
			 STUDIO_MODE_PROGRAM_SWITCH);
	assert_ptr_equal(s.program, intro);
	assert_ptr_equal(s.last_program, intro);
	assert_null(s.swap_scene);
	assert_null(s.swap_preview);
}

/* Guard: with "Duplicate Scene" on, a request for the program scene still
 * switches (to a copy of it); with swap off, no swap is scheduled. */
static void test_duplicate_and_no_swap(void **state)
{
	(void)state;
	struct studio_mode_program s = {intro, NULL, NULL, NULL};

	assert_int_equal(studio_mode_program_switch(&s, intro, other, false, true, true, false, false),
			 STUDIO_MODE_PROGRAM_SWITCH);
	assert_ptr_equal(s.program, intro);
	assert_ptr_equal(s.last_program, intro);
	assert_null(s.swap_scene);
	assert_null(s.swap_preview);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_refused_switch_keeps_program), cmocka_unit_test(test_refused_switch_keeps_swap),
		cmocka_unit_test(test_switch_after_transition),      cmocka_unit_test(test_same_scene),
		cmocka_unit_test(test_tbar_keeps_last_program),      cmocka_unit_test(test_forced_switch),
		cmocka_unit_test(test_duplicate_and_no_swap),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
