/* Characterize real public depth-state exports through a recording backend.
 * No graphics module, GPU allocation, or rendering is needed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

static struct graphics_subsystem graphics;
static unsigned char devices[2];
static gs_device_t *expected_device;
static unsigned enable_calls, function_calls, logs;
static bool last_enable;
static enum gs_depth_test last_function;
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

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, expected_device);
}

static void backend_enable(gs_device_t *device, bool enable)
{
	assert_ptr_equal(device, expected_device);
	enable_calls++;
	last_enable = enable;
}

static void backend_function(gs_device_t *device, enum gs_depth_test test)
{
	assert_ptr_equal(device, expected_device);
	function_calls++;
	last_function = test;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	enable_calls = function_calls = logs = 0;
	expected_device = (gs_device_t *)&devices[0];
	graphics.device = expected_device;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_enable_depth_test = backend_enable;
	graphics.exports.device_depth_function = backend_function;
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

static void diagnostic(const char *name)
{
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: called while not in a graphics context", name);
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_DEBUG);
	assert_string_equal(last_log, expected);
	assert_int_equal(enable_calls, 0);
	assert_int_equal(function_calls, 0);
	logs = 0;
}

static const enum gs_depth_test functions[] = {GS_NEVER,  GS_LESS,    GS_LEQUAL,   GS_EQUAL,
					       GS_GEQUAL, GS_GREATER, GS_NOTEQUAL, GS_ALWAYS};

static void missing_context(void **state)
{
	(void)state;
	/* Check both initially absent and released contexts, with callbacks installed. */
	for (unsigned phase = 0; phase < 2; phase++) {
		if (phase) {
			gs_enter_context(&graphics);
			gs_leave_context();
		}
		assert_null(gs_get_context());
		for (unsigned i = 0; i < 2; i++) {
			gs_enable_depth_test(i != 0);
			diagnostic("gs_enable_depth_test");
		}
		for (unsigned i = 0; i < sizeof(functions) / sizeof(functions[0]); i++) {
			gs_depth_function(functions[i]);
			diagnostic("gs_depth_function");
		}
	}
}

static void forwarding(void **state)
{
	(void)state;
	const bool enables[] = {true, true, false, false, true, false, true, false};
	_Static_assert(sizeof(enum gs_depth_test) == sizeof(int), "depth enum ABI size");
	_Static_assert(_Alignof(enum gs_depth_test) == _Alignof(int), "depth enum ABI alignment");
	for (unsigned device = 0; device < 2; device++) {
		expected_device = (gs_device_t *)&devices[device];
		graphics.device = expected_device;
		gs_enter_context(&graphics);
		assert_ptr_equal(gs_get_context(), &graphics);
		for (unsigned i = 0; i < sizeof(functions) / sizeof(functions[0]); i++) {
			/* Numeric values and repeated states are part of the characterized ABI. */
			assert_int_equal(functions[i], i);
			unsigned before_enable = enable_calls;
			unsigned before_function = function_calls;
			gs_enable_depth_test(enables[i]);
			assert_int_equal(last_enable, enables[i]);
			assert_int_equal(enable_calls, before_enable + 1);
			assert_int_equal(function_calls, before_function);
			gs_depth_function(functions[i]);
			assert_int_equal(last_function, functions[i]);
			assert_int_equal(function_calls, before_function + 1);
			assert_int_equal(enable_calls, before_enable + 1);
			/* No wrapper-side filtering, caching or implicit enable on function changes. */
			gs_depth_function(functions[i]);
			assert_int_equal(last_function, functions[i]);
			assert_int_equal(function_calls, before_function + 2);
			assert_int_equal(enable_calls, before_enable + 1);
			assert_int_equal(logs, 0);
		}
		gs_leave_context();
		assert_null(gs_get_context());
	}
	assert_int_equal(enable_calls, 16);
	assert_int_equal(function_calls, 32);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(missing_context, setup, teardown),
					   cmocka_unit_test_setup_teardown(forwarding, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
