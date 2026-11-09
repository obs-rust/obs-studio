/* Public-API stencil dispatch characterization (#4418); no GPU or display. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include "graphics/graphics-internal.h"
#include "util/base.h"

static int device_token;
static unsigned calls;
static unsigned logs;
static int operation;
static bool enabled;
static enum gs_stencil_side recorded_side;
static enum gs_depth_test recorded_test;
static enum gs_stencil_op_type recorded_ops[3];
static char log_text[128];

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	assert_int_equal(level, LOG_DEBUG);
	logs++;
	vsnprintf(log_text, sizeof(log_text), format, args);
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static void record_test(gs_device_t *device, bool enable)
{
	context(device);
	calls++;
	operation = 1;
	enabled = enable;
}

static void record_write(gs_device_t *device, bool enable)
{
	context(device);
	calls++;
	operation = 2;
	enabled = enable;
}

static void record_function(gs_device_t *device, enum gs_stencil_side side, enum gs_depth_test test)
{
	context(device);
	calls++;
	operation = 3;
	recorded_side = side;
	recorded_test = test;
}

static void record_op(gs_device_t *device, enum gs_stencil_side side, enum gs_stencil_op_type fail,
		      enum gs_stencil_op_type zfail, enum gs_stencil_op_type zpass)
{
	context(device);
	calls++;
	operation = 4;
	recorded_side = side;
	recorded_ops[0] = fail;
	recorded_ops[1] = zfail;
	recorded_ops[2] = zpass;
}

static void without_context(void)
{
	const char *names[] = {"gs_enable_stencil_test", "gs_enable_stencil_write", "gs_stencil_function",
			       "gs_stencil_op"};
	unsigned before = calls;
	unsigned prior_logs = logs;
	assert_null(gs_get_context());
	for (unsigned i = 0; i < 4; i++) {
		char expected[128];
		switch (i) {
		case 0:
			gs_enable_stencil_test(true);
			break;
		case 1:
			gs_enable_stencil_write(false);
			break;
		case 2:
			gs_stencil_function(GS_STENCIL_BACK, GS_LESS);
			break;
		case 3:
			gs_stencil_op(GS_STENCIL_BOTH, GS_REPLACE, GS_INCR, GS_DECR);
			break;
		}
		snprintf(expected, sizeof(expected), "%s: called while not in a graphics context", names[i]);
		assert_string_equal(log_text, expected);
		assert_int_equal(logs, prior_logs + i + 1);
		assert_int_equal(calls, before);
	}
}

static void dispatch(void **state)
{
	(void)state;
	struct graphics_subsystem graphics = {0};
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_enable_stencil_test = record_test;
	graphics.exports.device_enable_stencil_write = record_write;
	graphics.exports.device_stencil_function = record_function;
	graphics.exports.device_stencil_op = record_op;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_set_log_handler(capture_log, NULL);
	without_context();
	gs_enter_context(&graphics);
	unsigned expected_calls = calls;
	for (unsigned value = 0; value < 2; value++) {
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			gs_enable_stencil_test(value != 0);
			assert_int_equal(calls, ++expected_calls);
			assert_int_equal(operation, 1);
			assert_int_equal(enabled, value != 0);
			gs_enable_stencil_write(value != 0);
			assert_int_equal(calls, ++expected_calls);
			assert_int_equal(operation, 2);
			assert_int_equal(enabled, value != 0);
		}
	}
	for (enum gs_stencil_side side = GS_STENCIL_FRONT; side <= GS_STENCIL_BOTH; side++) {
		for (enum gs_depth_test test = GS_NEVER; test <= GS_ALWAYS; test++) {
			gs_stencil_function(side, test);
			assert_int_equal(calls, ++expected_calls);
			assert_int_equal(operation, 3);
			assert_int_equal(recorded_side, side);
			assert_int_equal(recorded_test, test);
		}
		for (enum gs_stencil_op_type fail = GS_KEEP; fail <= GS_INVERT; fail++) {
			for (enum gs_stencil_op_type zfail = GS_KEEP; zfail <= GS_INVERT; zfail++) {
				for (enum gs_stencil_op_type zpass = GS_KEEP; zpass <= GS_INVERT; zpass++) {
					gs_stencil_op(side, fail, zfail, zpass);
					assert_int_equal(calls, ++expected_calls);
					assert_int_equal(operation, 4);
					assert_int_equal(recorded_side, side);
					assert_int_equal(recorded_ops[0], fail);
					assert_int_equal(recorded_ops[1], zfail);
					assert_int_equal(recorded_ops[2], zpass);
				}
			}
		}
	}
	assert_int_equal(logs, 4);
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_int_equal(graphics.ref, 1);
	gs_leave_context();
	without_context();
	assert_int_equal(logs, 8);
	base_set_log_handler(NULL, NULL);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(dispatch)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
