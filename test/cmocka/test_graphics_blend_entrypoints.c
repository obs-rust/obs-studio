/* Characterize the public blend entry points against actual libobs, without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

static unsigned calls, logs;
static unsigned trace[3];
static struct blend_state observed;
static char message[128];
#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture(int level, const char *format, va_list args, void *param)
{
	(void)param;
	assert_int_equal(level, LOG_DEBUG);
	logs++;
	vsnprintf(message, sizeof(message), format, args);
}
static void context(gs_device_t *device) { (void)device; }
static void enable(gs_device_t *device, bool value)
{
	(void)device;
	assert_true(calls < 3);
	trace[calls++] = 1;
	observed.enabled = value;
}
static void factors(gs_device_t *device, enum gs_blend_type sc, enum gs_blend_type dc,
		    enum gs_blend_type sa, enum gs_blend_type da)
{
	(void)device;
	assert_true(calls < 3);
	trace[calls++] = 2;
	observed.src_c = sc;
	observed.dest_c = dc;
	observed.src_a = sa;
	observed.dest_a = da;
}
static void operation(gs_device_t *device, enum gs_blend_op_type value)
{
	(void)device;
	assert_true(calls < 3);
	trace[calls++] = 3;
	observed.op = value;
}
static void equal_state(struct blend_state a, struct blend_state b)
{
	assert_int_equal(a.enabled, b.enabled);
	assert_int_equal(a.src_c, b.src_c);
	assert_int_equal(a.dest_c, b.dest_c);
	assert_int_equal(a.src_a, b.src_a);
	assert_int_equal(a.dest_a, b.dest_a);
	assert_int_equal(a.op, b.op);
}
static void entrypoints(void **unused)
{
	(void)unused;
	struct graphics_subsystem g = {0};
	g.exports.device_enter_context = context;
	g.exports.device_leave_context = context;
	g.exports.device_enable_blending = enable;
	g.exports.device_blend_function_separate = factors;
	g.exports.device_blend_op = operation;
	assert_int_equal(pthread_mutex_init(&g.mutex, NULL), 0);
	base_set_log_handler(capture, NULL);
	gs_blend_state_push();
	assert_string_equal(message, "gs_blend_state_push: called while not in a graphics context");
	gs_blend_state_pop();
	assert_string_equal(message, "gs_blend_state_pop: called while not in a graphics context");
	gs_reset_blend_state();
	assert_string_equal(message, "gs_reset_blend_state: called while not in a graphics context");
	assert_int_equal(logs, 3);
	assert_int_equal(calls, 0);
	gs_enter_context(&g);
	gs_blend_state_pop();
	assert_int_equal(calls, 0);
	const struct blend_state defaults = {true, GS_BLEND_SRCALPHA, GS_BLEND_INVSRCALPHA,
		GS_BLEND_ONE, GS_BLEND_INVSRCALPHA, GS_BLEND_OP_ADD};
	const struct blend_state saved = {false, GS_BLEND_ONE, GS_BLEND_ZERO,
		GS_BLEND_SRCALPHA, GS_BLEND_ONE, GS_BLEND_OP_SUBTRACT};
	g.cur_blend_state = saved;
	gs_blend_state_push();
	assert_int_equal(g.blend_state_stack.num, 1);
	assert_int_equal(calls, 0);
	g.cur_blend_state = defaults;
	gs_blend_state_push();
	assert_int_equal(g.blend_state_stack.num, 2);
	gs_blend_state_pop();
	equal_state(observed, defaults);
	assert_int_equal(calls, 3);
	assert_int_equal(trace[0], 1);
	assert_int_equal(trace[1], 2);
	assert_int_equal(trace[2], 3);
	calls = 0;
	gs_blend_state_pop();
	equal_state(observed, saved);
	equal_state(g.cur_blend_state, saved);
	assert_int_equal(calls, 3);
	assert_int_equal(trace[0], 1);
	assert_int_equal(trace[1], 2);
	assert_int_equal(trace[2], 3);
	calls = 0;
	gs_blend_state_pop();
	assert_int_equal(calls, 0);
	/* All combinations of the three independently conditional reset calls. */
	for (unsigned mask = 0; mask < 8; mask++) {
		g.cur_blend_state = defaults;
		if (mask & 1) g.cur_blend_state.enabled = false;
		if (mask & 2) g.cur_blend_state.src_c = GS_BLEND_ZERO;
		if (mask & 4) g.cur_blend_state.op = GS_BLEND_OP_SUBTRACT;
		calls = 0;
		gs_reset_blend_state();
		unsigned n = 0;
		if (mask & 1) assert_int_equal(trace[n++], 1);
		if (mask & 2) assert_int_equal(trace[n++], 2);
		/* Characterized: changing factors also forces the ADD callback. */
		if (mask & 6) assert_int_equal(trace[n++], 3);
		assert_int_equal(calls, n);
		equal_state(g.cur_blend_state, defaults);
		calls = 0;
		gs_reset_blend_state();
		assert_int_equal(calls, 0);
	}
	assert_int_equal(logs, 3);
	gs_leave_context();
	da_free(g.blend_state_stack);
	base_set_log_handler(NULL, NULL);
	assert_int_equal(pthread_mutex_destroy(&g.mutex), 0);
}
int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(entrypoints)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
