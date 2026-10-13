#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include "studio-mode-swap.h"

/* Upstream obsproject/obs-websocket#1320 (reported by WORMSS): in Studio
 * Mode, SetCurrentProgramScene followed by SetCurrentPreviewScene (one after
 * the other, or in one request batch) leaves the old program scene in the
 * preview instead of the one just set.
 *
 * With "Swap Preview/Program Scenes After Transitioning" on (the default),
 * OBSBasic::TransitionToScene() remembers the scene that was on the program,
 * and OBSBasic::TransitionStopped() puts it in the preview when the
 * transition ends, 300 ms later with the default Fade. A preview chosen in
 * the meantime is overwritten.
 *
 * The model below stands in for OBSBasic: transition_start() is
 * TransitionToScene() (it schedules the swap under the same condition),
 * set_preview() is SetCurrentPreviewScene / a click in the scene list, and
 * transition_stop() is TransitionStopped(), which asks the helper. */

static char scene_a[] = "A";
static char scene_b[] = "B";
static char scene_c[] = "C";
static char scene_d[] = "D";

struct studio_model {
	char *program;
	char *preview;
	char *swap_scene;
	char *preview_at_start;
};

static void model_init(struct studio_model *m, char *program, char *preview)
{
	memset(m, 0, sizeof(*m));
	m->program = program;
	m->preview = preview;
}

static void transition_start(struct studio_model *m, char *target)
{
	char *last_program = m->program;

	if (last_program != target && last_program != m->preview) {
		m->swap_scene = last_program;
		m->preview_at_start = m->preview;
	}
	m->program = target;
}

static void set_preview(struct studio_model *m, char *scene)
{
	m->preview = scene;
}

static void transition_stop(struct studio_model *m)
{
	char *to = studio_mode_swap_scene(m->swap_scene, m->preview_at_start, m->preview);

	if (to) {
		m->preview = to;
	}
	m->swap_scene = NULL;
	m->preview_at_start = NULL;
}

/* The report: program C, preview D; SetCurrentProgramScene A, then
 * SetCurrentPreviewScene B while the transition runs. */
static void test_preview_set_during_transition(void **state)
{
	(void)state;
	struct studio_model m;
	model_init(&m, scene_c, scene_d);

	transition_start(&m, scene_a);
	set_preview(&m, scene_b);
	transition_stop(&m);

	printf("program %s, preview set to B during the transition, preview after it: %s\n", m.program, m.preview);

	assert_ptr_equal(m.program, scene_a);
	assert_ptr_equal(m.preview, scene_b);
}

/* Guard: the Transition button (preview A goes to the program) still swaps:
 * the old program C ends up in the preview. */
static void test_transition_button_swaps(void **state)
{
	(void)state;
	struct studio_model m;
	model_init(&m, scene_c, scene_a);

	transition_start(&m, scene_a);
	transition_stop(&m);

	assert_ptr_equal(m.program, scene_a);
	assert_ptr_equal(m.preview, scene_c);
}

/* Guard: SetCurrentProgramScene alone keeps today's behaviour: the old
 * program goes to the preview. */
static void test_program_only_swaps(void **state)
{
	(void)state;
	struct studio_model m;
	model_init(&m, scene_c, scene_d);

	transition_start(&m, scene_a);
	transition_stop(&m);

	assert_ptr_equal(m.program, scene_a);
	assert_ptr_equal(m.preview, scene_c);
}

/* Guard: nothing to swap (the old program scene is gone): the preview stays,
 * whatever happened during the transition. */
static void test_no_swap_scene(void **state)
{
	(void)state;
	assert_null(studio_mode_swap_scene(NULL, scene_d, scene_d));
	assert_null(studio_mode_swap_scene(NULL, scene_d, scene_b));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_preview_set_during_transition),
		cmocka_unit_test(test_transition_button_swaps),
		cmocka_unit_test(test_program_only_swaps),
		cmocka_unit_test(test_no_swap_scene),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
