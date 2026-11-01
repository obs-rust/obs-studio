#include <stdarg.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <float.h>
#include <limits.h>
#include <math.h>
#include <setjmp.h>
#include <cmocka.h>

#include "obs-core-setup-rules.h"

/* Characterization of the task queue's route and the video level checks,
 * moved from obs_queue_task(), obs_in_task_thread() and
 * obs_set_video_levels() in libobs/obs.c to
 * libobs/obs-core-setup-rules-task.c. The test calls only the moved
 * functions, which are hidden in libobs, so this characterizes that C
 * source. It pins the CURRENT behaviour; cases marked "characterized, not
 * endorsed" are odd but real. It never runs a task or a wait: only the
 * route is pinned. */

#define TASK(x) OBS_CORE_SETUP_RULES_TASK_##x

/* The types the route knows, and three it does not. */
static const int known[] = {TASK(UI), TASK(GRAPHICS), TASK(AUDIO), TASK(DESTROY)};
static const int unknown[] = {-1, 4, INT_MAX};

/* The steps obs_queue_task() walks, in its order, each asked with the
 * value it reads there; *asked gets one bit per step reached. */
static enum obs_core_setup_rules_task walk(int type, bool handler, bool in_thread, bool wait, unsigned *asked)
{
	enum obs_core_setup_rules_task step = obs_core_setup_rules_task_step(TASK(TYPE), type, false);
	*asked = 0;
	if (step == TASK(UI_HANDLER)) {
		*asked |= 1u << TASK(UI_HANDLER);
		step = obs_core_setup_rules_task_step(step, type, handler);
	}
	if (step == TASK(IN_THREAD)) {
		*asked |= 1u << TASK(IN_THREAD);
		step = obs_core_setup_rules_task_step(step, type, in_thread);
	}
	if (step == TASK(WAIT)) {
		*asked |= 1u << TASK(WAIT);
		step = obs_core_setup_rules_task_step(step, type, wait);
	}
	return step;
}

static void test_constants(void **state)
{
	(void)state;
	assert_int_equal(TASK(UI), 0);
	assert_int_equal(TASK(GRAPHICS), 1);
	assert_int_equal(TASK(AUDIO), 2);
	assert_int_equal(TASK(DESTROY), 3);

	assert_int_equal(TASK(TYPE), 0);
	assert_int_equal(TASK(UI_HANDLER), 1);
	assert_int_equal(TASK(IN_THREAD), 2);
	assert_int_equal(TASK(WAIT), 3);
	assert_int_equal(TASK(UI_CALL), 4);
	assert_int_equal(TASK(UI_NONE), 5);
	assert_int_equal(TASK(RUN), 6);
	assert_int_equal(TASK(WAIT_QUEUE), 7);
	assert_int_equal(TASK(PUSH_GRAPHICS), 8);
	assert_int_equal(TASK(PUSH_AUDIO), 9);
	assert_int_equal(TASK(PUSH_DESTROY), 10);
	assert_int_equal(TASK(DROP), 11);
	assert_int_equal(TASK(UNKNOWN), 12);
}

/* Each step on its own, for every type and both values. */
static void test_each_step(void **state)
{
	(void)state;
	for (int v = 0; v < 2; v++) {
		assert_int_equal(obs_core_setup_rules_task_step(TASK(TYPE), TASK(UI), v), TASK(UI_HANDLER));
		for (size_t i = 1; i < 4; i++)
			assert_int_equal(obs_core_setup_rules_task_step(TASK(TYPE), known[i], v), TASK(IN_THREAD));
		/* Characterized, not endorsed: an unknown type goes on to ask
		 * obs_in_task_thread(), which asserts. */
		for (size_t i = 0; i < 3; i++)
			assert_int_equal(obs_core_setup_rules_task_step(TASK(TYPE), unknown[i], v), TASK(IN_THREAD));
	}

	/* The handler, the thread and the wait steps answer the same for any
	 * type, but the wait step's queue. */
	static const int all[] = {TASK(UI), TASK(GRAPHICS), TASK(AUDIO), TASK(DESTROY), -1, 4, INT_MAX};
	for (size_t i = 0; i < sizeof(all) / sizeof(all[0]); i++) {
		int t = all[i];
		assert_int_equal(obs_core_setup_rules_task_step(TASK(UI_HANDLER), t, true), TASK(UI_CALL));
		assert_int_equal(obs_core_setup_rules_task_step(TASK(UI_HANDLER), t, false), TASK(UI_NONE));
		assert_int_equal(obs_core_setup_rules_task_step(TASK(IN_THREAD), t, true), TASK(RUN));
		assert_int_equal(obs_core_setup_rules_task_step(TASK(IN_THREAD), t, false), TASK(WAIT));
		assert_int_equal(obs_core_setup_rules_task_step(TASK(WAIT), t, true), TASK(WAIT_QUEUE));
	}
	assert_int_equal(obs_core_setup_rules_task_step(TASK(WAIT), TASK(GRAPHICS), false), TASK(PUSH_GRAPHICS));
	assert_int_equal(obs_core_setup_rules_task_step(TASK(WAIT), TASK(AUDIO), false), TASK(PUSH_AUDIO));
	assert_int_equal(obs_core_setup_rules_task_step(TASK(WAIT), TASK(DESTROY), false), TASK(PUSH_DESTROY));
	/* UI never reaches the wait step; asked anyway, it is not a queue. */
	assert_int_equal(obs_core_setup_rules_task_step(TASK(WAIT), TASK(UI), false), TASK(DROP));
	for (size_t i = 0; i < 3; i++)
		assert_int_equal(obs_core_setup_rules_task_step(TASK(WAIT), unknown[i], false), TASK(DROP));

	/* A route comes back unchanged. */
	for (int r = TASK(UI_CALL); r <= TASK(UNKNOWN); r++)
		for (size_t i = 0; i < sizeof(all) / sizeof(all[0]); i++)
			for (int v = 0; v < 2; v++)
				assert_int_equal(obs_core_setup_rules_task_step((enum obs_core_setup_rules_task)r,
										all[i], v),
						 r);
}

/* The whole walk: which steps each type asks, and where it ends. */
static void test_routes(void **state)
{
	(void)state;
	unsigned asked;
	const unsigned ui = 1u << TASK(UI_HANDLER);
	const unsigned thread = 1u << TASK(IN_THREAD);
	const unsigned thread_wait = thread | (1u << TASK(WAIT));

	for (int m = 0; m < 8; m++) {
		bool handler = m & 1, in_thread = m & 2, wait = m & 4;

		/* A UI task reads only the handler. */
		assert_int_equal(walk(TASK(UI), handler, in_thread, wait, &asked),
				 handler ? TASK(UI_CALL) : TASK(UI_NONE));
		assert_int_equal(asked, ui);

		static const int queue[] = {TASK(PUSH_GRAPHICS), TASK(PUSH_AUDIO), TASK(PUSH_DESTROY)};
		for (size_t i = 1; i < 4; i++) {
			int want = in_thread ? TASK(RUN) : wait ? TASK(WAIT_QUEUE) : queue[i - 1];
			assert_int_equal(walk(known[i], handler, in_thread, wait, &asked), want);
			assert_int_equal(asked, in_thread ? thread : thread_wait);
		}

		/* Characterized, not endorsed: an unknown type asks
		 * obs_in_task_thread() (which asserts and answers false), then
		 * waits for a task it never queues, or is dropped silently. */
		for (size_t i = 0; i < 3; i++) {
			int want = in_thread ? TASK(RUN) : wait ? TASK(WAIT_QUEUE) : TASK(DROP);
			assert_int_equal(walk(unknown[i], handler, in_thread, wait, &asked), want);
			assert_int_equal(asked, in_thread ? thread : thread_wait);
		}
	}
}

static void test_task_thread(void **state)
{
	(void)state;
	for (size_t i = 0; i < 4; i++)
		assert_int_equal(obs_core_setup_rules_task_thread(known[i]), known[i]);
	for (size_t i = 0; i < 3; i++)
		assert_int_equal(obs_core_setup_rules_task_thread(unknown[i]), -1);
	assert_int_equal(obs_core_setup_rules_task_thread(INT_MIN), -1);
	assert_int_equal(obs_core_setup_rules_task_thread(-2), -1);
}

/* Characterized, not endorsed: obs_set_video_levels() goes on with no
 * core, and reads obs->video through NULL. */
static void test_levels_core(void **state)
{
	(void)state;
	assert_true(obs_core_setup_rules_levels_core(true));
	assert_true(obs_core_setup_rules_levels_core(false));
}

static void test_level_ok(void **state)
{
	(void)state;
	assert_false(obs_core_setup_rules_level_ok(0.f));
	assert_false(obs_core_setup_rules_level_ok(-0.f));
	assert_true(obs_core_setup_rules_level_ok(1e-45f)); /* the smallest denormal */
	assert_true(obs_core_setup_rules_level_ok(FLT_MIN));
	assert_true(obs_core_setup_rules_level_ok(1.f));
	assert_true(obs_core_setup_rules_level_ok(300.f));
	assert_true(obs_core_setup_rules_level_ok(1000.f));
	assert_true(obs_core_setup_rules_level_ok(FLT_MAX));
	assert_false(obs_core_setup_rules_level_ok(-1e-45f));
	assert_false(obs_core_setup_rules_level_ok(-300.f));
	assert_false(obs_core_setup_rules_level_ok(-FLT_MAX));
	assert_false(obs_core_setup_rules_level_ok(INFINITY));
	assert_false(obs_core_setup_rules_level_ok(-INFINITY));
	assert_false(obs_core_setup_rules_level_ok(NAN));
	assert_false(obs_core_setup_rules_level_ok(-NAN));
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(test_constants),   cmocka_unit_test(test_each_step),   cmocka_unit_test(test_routes),
		cmocka_unit_test(test_task_thread), cmocka_unit_test(test_levels_core), cmocka_unit_test(test_level_ok),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
