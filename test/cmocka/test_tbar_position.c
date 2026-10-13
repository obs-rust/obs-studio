#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>

#include "tbar-position.h"

/* obs_frontend_set_tbar_position() and obs_frontend_release_tbar() drive the
 * Studio Mode T-Bar for plugins: obs-websocket's SetTBarPosition (Companion,
 * hardware T-Bars) and Advanced Scene Switcher's "Modify T-Bar position" and
 * "Release T-Bar". Setting the T-Bar to the end and releasing it cancels the
 * transition ("Manual transition cancelled") instead of finishing it, and
 * obs_frontend_get_tbar_position() keeps returning 0.
 *
 * The model below stands in for OBSBasic's T-Bar:
 * - the slider is a QSlider with the range 0 to 1023 (T_BAR_PRECISION - 1):
 *   setValue() clamps to the range and emits valueChanged only when the value
 *   changes, which is connected to OBSBasic::TBarChanged;
 * - position_changed() is OBSBasic::TBarChanged: it starts the manual
 *   transition and sets its progress;
 * - release() is OBSBasic::TBarReleased: it reads the slider's value, which is
 *   also what obs_frontend_get_tbar_position() returns. */

#define T_BAR_PRECISION 1024
#define T_BAR_MAX (T_BAR_PRECISION - 1)
#define T_BAR_CLAMP (T_BAR_PRECISION / 10)

struct tbar_model {
	int slider;              /* QSlider::value() */
	bool transition_active;  /* tBarActive */
	int transition_position; /* last value given to TBarChanged */
	int changes;
	struct tbar_position_ops ops;
};

static void model_position_changed(void *param, int value)
{
	struct tbar_model *m = param;
	m->transition_active = true;
	m->transition_position = value;
	m->changes++;
}

static void model_set_slider_value(void *param, int value)
{
	struct tbar_model *m = param;
	if (value < 0)
		value = 0;
	if (value > T_BAR_MAX)
		value = T_BAR_MAX;
	if (value == m->slider)
		return;
	m->slider = value;
	model_position_changed(m, value);
}

static void model_init(struct tbar_model *m)
{
	memset(m, 0, sizeof(*m));
	m->ops.param = m;
	m->ops.set_slider_value = model_set_slider_value;
	m->ops.position_changed = model_position_changed;
}

/* OBSBasic::TBarReleased */
static enum tbar_release_action model_release(struct tbar_model *m)
{
	enum tbar_release_action action = tbar_release_action(m->slider, T_BAR_MAX, T_BAR_CLAMP);
	if (action != TBAR_RELEASE_KEEP) {
		m->slider = 0; /* blockSignals(true); setValue(0) */
		m->transition_active = false;
	}
	return action;
}

/* obs-websocket SetTBarPosition {position: 1.0, release: true}: set the T-Bar
 * to the end, then release it. The transition has to finish. */
static void api_full_then_release_finishes(void **state)
{
	(void)state;
	struct tbar_model m;
	model_init(&m);

	tbar_api_set_position(&m.ops, T_BAR_PRECISION); /* round(1.0 * 1024) */
	assert_true(m.transition_active);
	assert_int_equal(model_release(&m), TBAR_RELEASE_FINISH);
	assert_false(m.transition_active);
}

/* A hardware T-Bar moved in steps without releasing, then released at the
 * end (Advanced Scene Switcher, Companion). */
static void api_steps_then_release_finishes(void **state)
{
	(void)state;
	struct tbar_model m;
	model_init(&m);

	tbar_api_set_position(&m.ops, 256);
	tbar_api_set_position(&m.ops, 512);
	tbar_api_set_position(&m.ops, 768);
	assert_int_equal(m.transition_position, 768);
	assert_int_equal(model_release(&m), TBAR_RELEASE_KEEP);
	assert_true(m.transition_active);

	tbar_api_set_position(&m.ops, 1023);
	assert_int_equal(model_release(&m), TBAR_RELEASE_FINISH);
}

/* obs_frontend_get_tbar_position() reads the slider: it has to return what
 * obs_frontend_set_tbar_position() set, and the UI has to show it. */
static void api_position_reaches_slider(void **state)
{
	(void)state;
	struct tbar_model m;
	model_init(&m);

	tbar_api_set_position(&m.ops, 512);
	assert_int_equal(m.slider, 512);
	assert_int_equal(m.transition_position, 512);
	assert_int_equal(m.changes, 1);
}

/* Guards that pass before and after the fix: the user dragging the slider,
 * and a release near the start cancelling. */
static void user_drag_and_cancel(void **state)
{
	(void)state;
	struct tbar_model m;
	model_init(&m);

	model_set_slider_value(&m, 1000);
	assert_int_equal(model_release(&m), TBAR_RELEASE_FINISH);

	model_set_slider_value(&m, 500);
	assert_int_equal(model_release(&m), TBAR_RELEASE_KEEP);
	model_set_slider_value(&m, 50);
	assert_int_equal(model_release(&m), TBAR_RELEASE_CANCEL);
	assert_false(m.transition_active);

	assert_int_equal(tbar_release_action(T_BAR_MAX - T_BAR_CLAMP, T_BAR_MAX, T_BAR_CLAMP), TBAR_RELEASE_FINISH);
	assert_int_equal(tbar_release_action(T_BAR_CLAMP, T_BAR_MAX, T_BAR_CLAMP), TBAR_RELEASE_CANCEL);
	assert_int_equal(tbar_release_action(T_BAR_CLAMP + 1, T_BAR_MAX, T_BAR_CLAMP), TBAR_RELEASE_KEEP);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(api_full_then_release_finishes),
		cmocka_unit_test(api_steps_then_release_finishes),
		cmocka_unit_test(api_position_reaches_slider),
		cmocka_unit_test(user_drag_and_cancel),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
