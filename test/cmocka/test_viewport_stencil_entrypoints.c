/* Characterize the public stencil, viewport, scissor and projection exports
 * through a recording backend. No graphics module, GPU or display is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

enum call {
	CALL_ENABLE_STENCIL_TEST,
	CALL_ENABLE_STENCIL_WRITE,
	CALL_STENCIL_FUNCTION,
	CALL_STENCIL_OP,
	CALL_SET_VIEWPORT,
	CALL_GET_VIEWPORT,
	CALL_SET_SCISSOR_RECT,
	CALL_ORTHO,
	CALL_FRUSTUM,
	CALL_PROJECTION_PUSH,
	CALL_PROJECTION_POP,
};

#define MAX_EVENTS 16

struct event {
	enum call call;
	const gs_device_t *device;
	bool flag;
	int side;
	int args[4];
	float planes[6];
	const struct gs_rect *rect;
};

static struct graphics_subsystem graphics;
static unsigned char devices[2];
static gs_device_t *expected_device;
static struct event events[MAX_EVENTS];
static unsigned num_events;
/* What the recording device reports as its viewport. */
static struct gs_rect device_viewport;
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
	return event;
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, expected_device);
}

static void backend_enable_stencil_test(gs_device_t *device, bool enable)
{
	record(CALL_ENABLE_STENCIL_TEST, device)->flag = enable;
}

static void backend_enable_stencil_write(gs_device_t *device, bool enable)
{
	record(CALL_ENABLE_STENCIL_WRITE, device)->flag = enable;
}

static void backend_stencil_function(gs_device_t *device, enum gs_stencil_side side, enum gs_depth_test test)
{
	struct event *event = record(CALL_STENCIL_FUNCTION, device);
	event->side = (int)side;
	event->args[0] = (int)test;
}

static void backend_stencil_op(gs_device_t *device, enum gs_stencil_side side, enum gs_stencil_op_type fail,
			       enum gs_stencil_op_type zfail, enum gs_stencil_op_type zpass)
{
	struct event *event = record(CALL_STENCIL_OP, device);
	event->side = (int)side;
	event->args[0] = (int)fail;
	event->args[1] = (int)zfail;
	event->args[2] = (int)zpass;
}

static void backend_set_viewport(gs_device_t *device, int x, int y, int width, int height)
{
	struct event *event = record(CALL_SET_VIEWPORT, device);
	event->args[0] = x;
	event->args[1] = y;
	event->args[2] = width;
	event->args[3] = height;
	device_viewport.x = x;
	device_viewport.y = y;
	device_viewport.cx = width;
	device_viewport.cy = height;
}

static void backend_get_viewport(const gs_device_t *device, struct gs_rect *rect)
{
	record(CALL_GET_VIEWPORT, device)->rect = rect;
	*rect = device_viewport;
}

static void backend_set_scissor_rect(gs_device_t *device, const struct gs_rect *rect)
{
	struct event *event = record(CALL_SET_SCISSOR_RECT, device);
	event->rect = rect;
	if (rect) {
		event->args[0] = rect->x;
		event->args[1] = rect->y;
		event->args[2] = rect->cx;
		event->args[3] = rect->cy;
	}
}

static void record_planes(struct event *event, float left, float right, float top, float bottom, float znear,
			  float zfar)
{
	event->planes[0] = left;
	event->planes[1] = right;
	event->planes[2] = top;
	event->planes[3] = bottom;
	event->planes[4] = znear;
	event->planes[5] = zfar;
}

static void backend_ortho(gs_device_t *device, float left, float right, float top, float bottom, float znear,
			  float zfar)
{
	record_planes(record(CALL_ORTHO, device), left, right, top, bottom, znear, zfar);
}

static void backend_frustum(gs_device_t *device, float left, float right, float top, float bottom, float znear,
			    float zfar)
{
	record_planes(record(CALL_FRUSTUM, device), left, right, top, bottom, znear, zfar);
}

static void backend_projection_push(gs_device_t *device)
{
	record(CALL_PROJECTION_PUSH, device);
}

static void backend_projection_pop(gs_device_t *device)
{
	record(CALL_PROJECTION_POP, device);
}

static const struct gs_rect initial_viewport = {3, 5, 640, 360};

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	memset(events, 0, sizeof(events));
	num_events = logs = 0;
	device_viewport = initial_viewport;
	expected_device = (gs_device_t *)&devices[0];
	graphics.device = expected_device;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_enable_stencil_test = backend_enable_stencil_test;
	graphics.exports.device_enable_stencil_write = backend_enable_stencil_write;
	graphics.exports.device_stencil_function = backend_stencil_function;
	graphics.exports.device_stencil_op = backend_stencil_op;
	graphics.exports.device_set_viewport = backend_set_viewport;
	graphics.exports.device_get_viewport = backend_get_viewport;
	graphics.exports.device_set_scissor_rect = backend_set_scissor_rect;
	graphics.exports.device_ortho = backend_ortho;
	graphics.exports.device_frustum = backend_frustum;
	graphics.exports.device_projection_push = backend_projection_push;
	graphics.exports.device_projection_pop = backend_projection_pop;
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
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void equal_rect(struct gs_rect a, struct gs_rect b)
{
	assert_int_equal(a.x, b.x);
	assert_int_equal(a.y, b.y);
	assert_int_equal(a.cx, b.cx);
	assert_int_equal(a.cy, b.cy);
}

static void logged(const char *expected)
{
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_DEBUG);
	assert_string_equal(last_log, expected);
	assert_int_equal(num_events, 0);
	logs = 0;
}

static void diagnostic(const char *name)
{
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: called while not in a graphics context", name);
	logged(expected);
}

static void missing_context(void **state)
{
	(void)state;
	/* Check both initially absent and released contexts, with every callback
	 * installed. */
	for (unsigned phase = 0; phase < 2; phase++) {
		if (phase) {
			gs_enter_context(&graphics);
			gs_leave_context();
		}
		assert_null(gs_get_context());

		gs_enable_stencil_test(true);
		diagnostic("gs_enable_stencil_test");
		gs_enable_stencil_write(false);
		diagnostic("gs_enable_stencil_write");
		gs_stencil_function(GS_STENCIL_BACK, GS_LESS);
		diagnostic("gs_stencil_function");
		gs_stencil_op(GS_STENCIL_BOTH, GS_REPLACE, GS_INCR, GS_DECR);
		diagnostic("gs_stencil_op");

		gs_set_viewport(1, 2, 3, 4);
		diagnostic("gs_set_viewport");
		/* The caller's rect is left as it was. */
		struct gs_rect rect = {-1, -2, -3, -4};
		gs_get_viewport(&rect);
		diagnostic("gs_get_viewport");
		equal_rect(rect, (struct gs_rect){-1, -2, -3, -4});
		/* The context check comes first: a NULL rect logs only that. */
		gs_get_viewport(NULL);
		diagnostic("gs_get_viewport");
		gs_set_scissor_rect(&rect);
		diagnostic("gs_set_scissor_rect");
		gs_set_scissor_rect(NULL);
		diagnostic("gs_set_scissor_rect");

		gs_ortho(0.0f, 1.0f, 0.0f, 1.0f, -100.0f, 100.0f);
		diagnostic("gs_ortho");
		gs_frustum(-1.0f, 1.0f, 1.0f, -1.0f, 0.1f, 1000.0f);
		diagnostic("gs_frustum");
		gs_projection_push();
		diagnostic("gs_projection_push");
		gs_projection_pop();
		diagnostic("gs_projection_pop");

		/* The device's viewport was never touched. */
		equal_rect(device_viewport, initial_viewport);
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

static void stencil_forwarding(void)
{
	for (unsigned value = 0; value < 2; value++) {
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			gs_enable_stencil_test(value != 0);
			assert_int_equal(expect_only(CALL_ENABLE_STENCIL_TEST)->flag, value != 0);
			gs_enable_stencil_write(value != 0);
			assert_int_equal(expect_only(CALL_ENABLE_STENCIL_WRITE)->flag, value != 0);
		}
	}

	/* The numeric values are part of the ABI the port must keep. */
	assert_int_equal(GS_STENCIL_FRONT, 1);
	assert_int_equal(GS_STENCIL_BOTH, 3);
	assert_int_equal(GS_NEVER, 0);
	assert_int_equal(GS_ALWAYS, 7);
	assert_int_equal(GS_KEEP, 0);
	assert_int_equal(GS_INVERT, 5);

	for (int side = GS_STENCIL_FRONT; side <= GS_STENCIL_BOTH; side++) {
		for (int test = GS_NEVER; test <= GS_ALWAYS; test++) {
			gs_stencil_function((enum gs_stencil_side)side, (enum gs_depth_test)test);
			struct event *event = expect_only(CALL_STENCIL_FUNCTION);
			assert_int_equal(event->side, side);
			assert_int_equal(event->args[0], test);
		}
		/* Every operation in every position keeps its place. */
		for (int fail = GS_KEEP; fail <= GS_INVERT; fail++) {
			for (int zfail = GS_KEEP; zfail <= GS_INVERT; zfail++) {
				for (int zpass = GS_KEEP; zpass <= GS_INVERT; zpass++) {
					gs_stencil_op((enum gs_stencil_side)side, (enum gs_stencil_op_type)fail,
						      (enum gs_stencil_op_type)zfail, (enum gs_stencil_op_type)zpass);
					struct event *event = expect_only(CALL_STENCIL_OP);
					assert_int_equal(event->side, side);
					assert_int_equal(event->args[0], fail);
					assert_int_equal(event->args[1], zfail);
					assert_int_equal(event->args[2], zpass);
				}
			}
		}
	}

	/* No wrapper-side validation: undeclared values pass through. */
	gs_stencil_function((enum gs_stencil_side)0, (enum gs_depth_test)99);
	struct event *event = expect_only(CALL_STENCIL_FUNCTION);
	assert_int_equal(event->side, 0);
	assert_int_equal(event->args[0], 99);
	gs_stencil_op((enum gs_stencil_side)7, (enum gs_stencil_op_type)8, (enum gs_stencil_op_type)6,
		      (enum gs_stencil_op_type)0x7fff);
	event = expect_only(CALL_STENCIL_OP);
	assert_int_equal(event->side, 7);
	assert_int_equal(event->args[0], 8);
	assert_int_equal(event->args[1], 6);
	assert_int_equal(event->args[2], 0x7fff);
}

static void viewport_forwarding(void)
{
	const struct gs_rect viewports[] = {
		{0, 0, 1920, 1080},
		{-16, 32, 0, 0},
		{INT32_MIN, INT32_MAX, -1, INT32_MIN},
	};
	for (unsigned v = 0; v < sizeof(viewports) / sizeof(viewports[0]); v++) {
		const struct gs_rect want = viewports[v];
		gs_set_viewport(want.x, want.y, want.cx, want.cy);
		struct event *event = expect_only(CALL_SET_VIEWPORT);
		/* Each argument keeps its position, unvalidated. */
		assert_int_equal(event->args[0], want.x);
		assert_int_equal(event->args[1], want.y);
		assert_int_equal(event->args[2], want.cx);
		assert_int_equal(event->args[3], want.cy);

		/* The getter asks the device each time, into the caller's own rect. */
		struct gs_rect rect = {0};
		gs_get_viewport(&rect);
		assert_ptr_equal(expect_only(CALL_GET_VIEWPORT)->rect, &rect);
		equal_rect(rect, want);
	}

	/* What the device reports is returned, not what was last set. */
	device_viewport = (struct gs_rect){7, 8, 9, 10};
	struct gs_rect rect = {0};
	gs_get_viewport(&rect);
	expect_only(CALL_GET_VIEWPORT);
	equal_rect(rect, device_viewport);

	/* A NULL rect inside a context is ptr_valid's line, and no dispatch. */
	gs_get_viewport(NULL);
	logged("gs_get_viewport: Null 'rect' parameter");

	/* The scissor rect is handed over as the caller's pointer, NULL included. */
	const struct gs_rect scissor = {11, -12, 13, 14};
	gs_set_scissor_rect(&scissor);
	struct event *event = expect_only(CALL_SET_SCISSOR_RECT);
	assert_ptr_equal(event->rect, &scissor);
	assert_int_equal(event->args[0], 11);
	assert_int_equal(event->args[1], -12);
	assert_int_equal(event->args[2], 13);
	assert_int_equal(event->args[3], 14);
	gs_set_scissor_rect(NULL);
	assert_null(expect_only(CALL_SET_SCISSOR_RECT)->rect);
}

static void equal_planes(const struct event *event, const float want[6])
{
	/* Bit for bit: signed zero, NaN and infinities pass through unchanged. */
	assert_memory_equal(event->planes, want, sizeof(float) * 6);
}

static void projection_forwarding(void)
{
	const float sets[][6] = {
		{0.0f, 1920.0f, 0.0f, 1080.0f, -100.0f, 100.0f},
		{-1.5f, 2.25f, 3.0f, -4.75f, 0.125f, 1000.0f},
		{-0.0f, INFINITY, -INFINITY, 1e-30f, 3.4e38f, -2.0f},
	};
	for (unsigned s = 0; s < sizeof(sets) / sizeof(sets[0]); s++) {
		const float *p = sets[s];
		gs_ortho(p[0], p[1], p[2], p[3], p[4], p[5]);
		equal_planes(expect_only(CALL_ORTHO), p);
		gs_frustum(p[0], p[1], p[2], p[3], p[4], p[5]);
		equal_planes(expect_only(CALL_FRUSTUM), p);
	}
	const float qnan = NAN;
	const float with_nan[6] = {qnan, 1.0f, qnan, 2.0f, qnan, 3.0f};
	gs_ortho(with_nan[0], with_nan[1], with_nan[2], with_nan[3], with_nan[4], with_nan[5]);
	equal_planes(expect_only(CALL_ORTHO), with_nan);

	/* Push and pop reach the device once each, in call order, around the
	 * projection they bracket. */
	const float inner[6] = {0.0f, 4.0f, 0.0f, 3.0f, -1.0f, 1.0f};
	gs_projection_push();
	gs_ortho(inner[0], inner[1], inner[2], inner[3], inner[4], inner[5]);
	gs_projection_push();
	gs_frustum(inner[0], inner[1], inner[2], inner[3], inner[4], inner[5]);
	gs_projection_pop();
	gs_projection_pop();
	const enum call order[] = {CALL_PROJECTION_PUSH, CALL_ORTHO,          CALL_PROJECTION_PUSH,
				   CALL_FRUSTUM,         CALL_PROJECTION_POP, CALL_PROJECTION_POP};
	assert_int_equal(num_events, 6);
	for (unsigned i = 0; i < 6; i++) {
		assert_int_equal(events[i].call, order[i]);
		assert_ptr_equal(events[i].device, expected_device);
	}
	equal_planes(&events[1], inner);
	equal_planes(&events[3], inner);
	assert_int_equal(logs, 0);
	num_events = 0;

	/* Pop is forwarded even with nothing pushed: the device owns the stack. */
	gs_projection_pop();
	expect_only(CALL_PROJECTION_POP);
}

static void forwarding(void **state)
{
	(void)state;
	_Static_assert(sizeof(enum gs_stencil_side) == sizeof(int), "stencil side enum ABI size");
	_Static_assert(sizeof(enum gs_depth_test) == sizeof(int), "depth test enum ABI size");
	_Static_assert(sizeof(enum gs_stencil_op_type) == sizeof(int), "stencil op enum ABI size");
	for (unsigned device = 0; device < 2; device++) {
		expected_device = (gs_device_t *)&devices[device];
		graphics.device = expected_device;
		gs_enter_context(&graphics);
		assert_ptr_equal(gs_get_context(), &graphics);

		stencil_forwarding();
		viewport_forwarding();
		projection_forwarding();

		assert_ptr_equal(gs_get_context(), &graphics);
		assert_int_equal(graphics.ref, 1);
		gs_leave_context();
		assert_int_equal(logs, 0);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(missing_context, setup, teardown),
		cmocka_unit_test_setup_teardown(forwarding, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
