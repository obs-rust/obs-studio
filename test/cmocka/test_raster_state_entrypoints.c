/* Characterize the public cull, colour-mask and blend setter exports through a
 * recording backend. No graphics module, GPU allocation, or rendering is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

enum call {
	CALL_SET_CULL_MODE,
	CALL_GET_CULL_MODE,
	CALL_ENABLE_BLENDING,
	CALL_ENABLE_COLOR,
	CALL_BLEND_FUNCTION,
	CALL_BLEND_FUNCTION_SEPARATE,
	CALL_BLEND_OP,
};

#define MAX_EVENTS 16

struct event {
	enum call call;
	const gs_device_t *device;
	int cull_mode;
	bool flag;
	bool rgba[4];
	int factors[4];
	int op;
	/* The subsystem's blend state as the export saw it when called. */
	struct blend_state seen;
};

static struct graphics_subsystem graphics;
static unsigned char devices[2];
static gs_device_t *expected_device;
static struct event events[MAX_EVENTS];
static unsigned num_events;
static enum gs_cull_mode cull_answer;
static unsigned logs;
static int last_level;
static char last_log[160];
static log_handler_t saved_handler;
static void *saved_param;

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	logs++;
	last_level = level;
	vsnprintf(last_log, sizeof(last_log), format, args);
}

static struct event *record(enum call call, const gs_device_t *device)
{
	assert_ptr_equal(device, expected_device);
	assert_true(num_events < MAX_EVENTS);
	struct event *event = &events[num_events++];
	memset(event, 0, sizeof(*event));
	event->call = call;
	event->device = device;
	event->seen = graphics.cur_blend_state;
	return event;
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, expected_device);
}

static void backend_set_cull_mode(gs_device_t *device, enum gs_cull_mode mode)
{
	record(CALL_SET_CULL_MODE, device)->cull_mode = (int)mode;
}

static enum gs_cull_mode backend_get_cull_mode(const gs_device_t *device)
{
	record(CALL_GET_CULL_MODE, device);
	return cull_answer;
}

static void backend_enable_blending(gs_device_t *device, bool enable)
{
	record(CALL_ENABLE_BLENDING, device)->flag = enable;
}

static void backend_enable_color(gs_device_t *device, bool red, bool green, bool blue, bool alpha)
{
	struct event *event = record(CALL_ENABLE_COLOR, device);
	event->rgba[0] = red;
	event->rgba[1] = green;
	event->rgba[2] = blue;
	event->rgba[3] = alpha;
}

static void backend_blend_function(gs_device_t *device, enum gs_blend_type src, enum gs_blend_type dest)
{
	struct event *event = record(CALL_BLEND_FUNCTION, device);
	event->factors[0] = (int)src;
	event->factors[1] = (int)dest;
}

static void backend_blend_function_separate(gs_device_t *device, enum gs_blend_type src_c, enum gs_blend_type dest_c,
					    enum gs_blend_type src_a, enum gs_blend_type dest_a)
{
	struct event *event = record(CALL_BLEND_FUNCTION_SEPARATE, device);
	event->factors[0] = (int)src_c;
	event->factors[1] = (int)dest_c;
	event->factors[2] = (int)src_a;
	event->factors[3] = (int)dest_a;
}

static void backend_blend_op(gs_device_t *device, enum gs_blend_op_type op)
{
	record(CALL_BLEND_OP, device)->op = (int)op;
}

static const struct blend_state initial = {
	false, GS_BLEND_DSTCOLOR, GS_BLEND_INVDSTCOLOR, GS_BLEND_DSTALPHA, GS_BLEND_INVDSTALPHA, GS_BLEND_OP_MAX};

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	memset(events, 0, sizeof(events));
	num_events = logs = 0;
	cull_answer = GS_FRONT;
	expected_device = (gs_device_t *)&devices[0];
	graphics.device = expected_device;
	graphics.cur_blend_state = initial;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_set_cull_mode = backend_set_cull_mode;
	graphics.exports.device_get_cull_mode = backend_get_cull_mode;
	graphics.exports.device_enable_blending = backend_enable_blending;
	graphics.exports.device_enable_color = backend_enable_color;
	graphics.exports.device_blend_function = backend_blend_function;
	graphics.exports.device_blend_function_separate = backend_blend_function_separate;
	graphics.exports.device_blend_op = backend_blend_op;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	base_set_log_handler(saved_handler, saved_param);
	da_free(graphics.blend_state_stack);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void equal_blend(struct blend_state a, struct blend_state b)
{
	assert_int_equal(a.enabled, b.enabled);
	assert_int_equal(a.src_c, b.src_c);
	assert_int_equal(a.dest_c, b.dest_c);
	assert_int_equal(a.src_a, b.src_a);
	assert_int_equal(a.dest_a, b.dest_a);
	assert_int_equal(a.op, b.op);
}

static void diagnostic(const char *name)
{
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: called while not in a graphics context", name);
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_DEBUG);
	assert_string_equal(last_log, expected);
	assert_int_equal(num_events, 0);
	/* Without a context nothing reaches the subsystem's blend state either. */
	equal_blend(graphics.cur_blend_state, initial);
	logs = 0;
}

static void missing_context(void **state)
{
	(void)state;
	/* Check both initially absent and released contexts, with callbacks installed
	 * and the backend's cull answer set to something other than GS_NEITHER. */
	for (unsigned phase = 0; phase < 2; phase++) {
		if (phase) {
			gs_enter_context(&graphics);
			gs_leave_context();
		}
		assert_null(gs_get_context());
		gs_set_cull_mode(GS_BACK);
		diagnostic("gs_set_cull_mode");
		assert_int_equal(gs_get_cull_mode(), GS_NEITHER);
		diagnostic("gs_get_cull_mode");
		gs_enable_blending(true);
		diagnostic("gs_enable_blending");
		gs_enable_color(true, false, true, false);
		diagnostic("gs_enable_color");
		gs_blend_function(GS_BLEND_ONE, GS_BLEND_ZERO);
		diagnostic("gs_blend_function");
		gs_blend_function_separate(GS_BLEND_ONE, GS_BLEND_ZERO, GS_BLEND_SRCALPHA, GS_BLEND_INVSRCALPHA);
		diagnostic("gs_blend_function_separate");
		gs_blend_op(GS_BLEND_OP_SUBTRACT);
		diagnostic("gs_blend_op");
	}
}

static struct event *expect_only(enum call call)
{
	assert_int_equal(num_events, 1);
	assert_int_equal(events[0].call, call);
	assert_ptr_equal(events[0].device, expected_device);
	assert_int_equal(logs, 0);
	num_events = 0;
	return &events[0];
}

static void cull_forwarding(void)
{
	const enum gs_cull_mode modes[] = {GS_BACK, GS_FRONT, GS_NEITHER};
	for (unsigned m = 0; m < sizeof(modes) / sizeof(modes[0]); m++) {
		/* The numeric values are part of the ABI the port must keep. */
		assert_int_equal(modes[m], m);
		gs_set_cull_mode(modes[m]);
		assert_int_equal(expect_only(CALL_SET_CULL_MODE)->cull_mode, modes[m]);
		/* The getter returns the export's answer, never a cached value. */
		cull_answer = modes[(m + 1) % 3];
		assert_int_equal(gs_get_cull_mode(), cull_answer);
		expect_only(CALL_GET_CULL_MODE);
	}
	/* No wrapper-side validation: undeclared values pass through both ways. */
	gs_set_cull_mode((enum gs_cull_mode)77);
	assert_int_equal(expect_only(CALL_SET_CULL_MODE)->cull_mode, 77);
	cull_answer = (enum gs_cull_mode)9;
	assert_int_equal(gs_get_cull_mode(), 9);
	expect_only(CALL_GET_CULL_MODE);
	/* Cull calls leave the blend state alone. */
	equal_blend(graphics.cur_blend_state, initial);
}

static void color_forwarding(void)
{
	for (unsigned mask = 0; mask < 16; mask++) {
		bool r = mask & 1, g = mask & 2, b = mask & 4, a = mask & 8;
		gs_enable_color(r, g, b, a);
		struct event *event = expect_only(CALL_ENABLE_COLOR);
		/* Each channel keeps its own position. */
		assert_int_equal(event->rgba[0], r);
		assert_int_equal(event->rgba[1], g);
		assert_int_equal(event->rgba[2], b);
		assert_int_equal(event->rgba[3], a);
	}
	equal_blend(graphics.cur_blend_state, initial);
}

static void blend_forwarding(void)
{
	struct blend_state expected = graphics.cur_blend_state;

	for (unsigned i = 0; i < 3; i++) {
		bool enable = i != 1;
		gs_enable_blending(enable);
		expected.enabled = enable;
		struct event *event = expect_only(CALL_ENABLE_BLENDING);
		assert_int_equal(event->flag, enable);
		/* The state is stored before the device is told. */
		equal_blend(event->seen, expected);
		equal_blend(graphics.cur_blend_state, expected);
	}

	const enum gs_blend_type pairs[][2] = {
		{GS_BLEND_ZERO, GS_BLEND_ONE},
		{GS_BLEND_SRCALPHA, GS_BLEND_INVSRCALPHA},
		{GS_BLEND_SRCALPHASAT, GS_BLEND_DSTCOLOR},
		{(enum gs_blend_type)42, (enum gs_blend_type)0x7fff},
	};
	for (unsigned p = 0; p < sizeof(pairs) / sizeof(pairs[0]); p++) {
		gs_blend_function(pairs[p][0], pairs[p][1]);
		/* One pair sets both the colour and the alpha factors. */
		expected.src_c = expected.src_a = pairs[p][0];
		expected.dest_c = expected.dest_a = pairs[p][1];
		struct event *event = expect_only(CALL_BLEND_FUNCTION);
		assert_int_equal(event->factors[0], pairs[p][0]);
		assert_int_equal(event->factors[1], pairs[p][1]);
		equal_blend(event->seen, expected);
		equal_blend(graphics.cur_blend_state, expected);
	}

	const enum gs_blend_type quads[][4] = {
		{GS_BLEND_ONE, GS_BLEND_ZERO, GS_BLEND_SRCALPHA, GS_BLEND_INVSRCALPHA},
		{GS_BLEND_SRCCOLOR, GS_BLEND_INVSRCCOLOR, GS_BLEND_DSTALPHA, GS_BLEND_INVDSTALPHA},
		{GS_BLEND_INVDSTCOLOR, GS_BLEND_DSTCOLOR, GS_BLEND_ONE, GS_BLEND_SRCALPHASAT},
		{(enum gs_blend_type)99, (enum gs_blend_type)300, (enum gs_blend_type)1000, GS_BLEND_ZERO},
	};
	for (unsigned q = 0; q < sizeof(quads) / sizeof(quads[0]); q++) {
		gs_blend_function_separate(quads[q][0], quads[q][1], quads[q][2], quads[q][3]);
		expected.src_c = quads[q][0];
		expected.dest_c = quads[q][1];
		expected.src_a = quads[q][2];
		expected.dest_a = quads[q][3];
		struct event *event = expect_only(CALL_BLEND_FUNCTION_SEPARATE);
		for (unsigned k = 0; k < 4; k++)
			assert_int_equal(event->factors[k], quads[q][k]);
		equal_blend(event->seen, expected);
		equal_blend(graphics.cur_blend_state, expected);
	}

	const enum gs_blend_op_type ops[] = {GS_BLEND_OP_ADD, GS_BLEND_OP_SUBTRACT, GS_BLEND_OP_REVERSE_SUBTRACT,
					     GS_BLEND_OP_MIN, GS_BLEND_OP_MAX,      (enum gs_blend_op_type)17};
	for (unsigned o = 0; o < sizeof(ops) / sizeof(ops[0]); o++) {
		gs_blend_op(ops[o]);
		expected.op = ops[o];
		struct event *event = expect_only(CALL_BLEND_OP);
		/* The export gets the operation as stored in the blend state. */
		assert_int_equal(event->op, ops[o]);
		equal_blend(event->seen, expected);
		equal_blend(graphics.cur_blend_state, expected);
	}
}

static void forwarding(void **state)
{
	(void)state;
	_Static_assert(sizeof(enum gs_cull_mode) == sizeof(int), "cull mode enum ABI size");
	_Static_assert(sizeof(enum gs_blend_type) == sizeof(int), "blend type enum ABI size");
	_Static_assert(sizeof(enum gs_blend_op_type) == sizeof(int), "blend op enum ABI size");
	for (unsigned device = 0; device < 2; device++) {
		expected_device = (gs_device_t *)&devices[device];
		graphics.device = expected_device;
		graphics.cur_blend_state = initial;
		gs_enter_context(&graphics);
		assert_ptr_equal(gs_get_context(), &graphics);

		cull_forwarding();
		color_forwarding();
		blend_forwarding();

		gs_leave_context();
		assert_int_equal(logs, 0);
	}
}

static void push_pop_sees_setters(void **state)
{
	(void)state;
	gs_enter_context(&graphics);

	/* What the setters store is what push saves ... */
	gs_enable_blending(true);
	gs_blend_function_separate(GS_BLEND_ONE, GS_BLEND_ZERO, GS_BLEND_SRCALPHA, GS_BLEND_INVSRCALPHA);
	gs_blend_op(GS_BLEND_OP_REVERSE_SUBTRACT);
	const struct blend_state saved = {true,
					  GS_BLEND_ONE,
					  GS_BLEND_ZERO,
					  GS_BLEND_SRCALPHA,
					  GS_BLEND_INVSRCALPHA,
					  GS_BLEND_OP_REVERSE_SUBTRACT};
	equal_blend(graphics.cur_blend_state, saved);
	gs_blend_state_push();
	assert_int_equal(graphics.blend_state_stack.num, 1);

	/* ... and pop restores it after the setters change it. */
	gs_enable_blending(false);
	gs_blend_function(GS_BLEND_DSTCOLOR, GS_BLEND_INVDSTCOLOR);
	gs_blend_op(GS_BLEND_OP_MIN);
	num_events = 0;
	gs_blend_state_pop();
	assert_int_equal(graphics.blend_state_stack.num, 0);
	equal_blend(graphics.cur_blend_state, saved);
	assert_int_equal(num_events, 3);
	assert_int_equal(events[0].call, CALL_ENABLE_BLENDING);
	assert_int_equal(events[0].flag, true);
	assert_int_equal(events[1].call, CALL_BLEND_FUNCTION_SEPARATE);
	assert_int_equal(events[1].factors[0], GS_BLEND_ONE);
	assert_int_equal(events[1].factors[1], GS_BLEND_ZERO);
	assert_int_equal(events[1].factors[2], GS_BLEND_SRCALPHA);
	assert_int_equal(events[1].factors[3], GS_BLEND_INVSRCALPHA);
	assert_int_equal(events[2].call, CALL_BLEND_OP);
	assert_int_equal(events[2].op, GS_BLEND_OP_REVERSE_SUBTRACT);
	assert_int_equal(logs, 0);

	/* gs_blend_function's single pair is pushed as colour and alpha alike. */
	gs_blend_function(GS_BLEND_SRCCOLOR, GS_BLEND_INVSRCCOLOR);
	gs_blend_state_push();
	gs_blend_function_separate(GS_BLEND_ZERO, GS_BLEND_ZERO, GS_BLEND_ZERO, GS_BLEND_ZERO);
	num_events = 0;
	gs_blend_state_pop();
	assert_int_equal(events[1].call, CALL_BLEND_FUNCTION_SEPARATE);
	assert_int_equal(events[1].factors[0], GS_BLEND_SRCCOLOR);
	assert_int_equal(events[1].factors[1], GS_BLEND_INVSRCCOLOR);
	assert_int_equal(events[1].factors[2], GS_BLEND_SRCCOLOR);
	assert_int_equal(events[1].factors[3], GS_BLEND_INVSRCCOLOR);

	gs_leave_context();
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(missing_context, setup, teardown),
		cmocka_unit_test_setup_teardown(forwarding, setup, teardown),
		cmocka_unit_test_setup_teardown(push_pop_sees_setters, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
