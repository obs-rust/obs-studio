/* Characterize optional planar capability callbacks without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

/* This characterization lands with the extraction, before the Rust swap. */
#if !__has_include("graphics/graphics-planar-capability-queries.c.h")
#error "graphics planar capability query extraction seam is missing"
#endif

static struct graphics_subsystem graphics;
static int device_tokens[2];
static gs_device_t *expected_device;
static bool nv12_result, p010_result;
static unsigned nv12_calls, p010_calls, log_count;
static char message[160];
static log_handler_t previous_handler;
static void *previous_param;

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture(int level, const char *format, va_list args, void *param)
{
	(void)param;
	assert_int_equal(level, LOG_DEBUG);
	log_count++;
	vsnprintf(message, sizeof(message), format, args);
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, expected_device);
}

static bool nv12(gs_device_t *device)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(device, expected_device);
	nv12_calls++;
	return nv12_result;
}

static bool p010(gs_device_t *device)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(device, expected_device);
	p010_calls++;
	return p010_result;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	nv12_calls = p010_calls = log_count = 0;
	nv12_result = p010_result = false;
	message[0] = '\0';
	expected_device = (gs_device_t *)&device_tokens[0];
	graphics.device = expected_device;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_nv12_available = nv12;
	graphics.exports.device_p010_available = p010;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_get_log_handler(&previous_handler, &previous_param);
	base_set_log_handler(capture, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	base_set_log_handler(previous_handler, previous_param);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void no_context(void **state)
{
	(void)state;
	/* Context rejection precedes optional callback inspection. */
	for (unsigned i = 0; i < 2; i++) {
		assert_false(gs_nv12_available());
		assert_string_equal(message, "gs_nv12_available: called while not in a graphics context");
		assert_int_equal(log_count, 2 * i + 1);
		assert_false(gs_p010_available());
		assert_string_equal(message, "gs_p010_available: called while not in a graphics context");
		assert_int_equal(log_count, 2 * i + 2);
		assert_int_equal(nv12_calls, 0);
		assert_int_equal(p010_calls, 0);
		graphics.exports.device_nv12_available = NULL;
		graphics.exports.device_p010_available = NULL;
	}
}

static void optional_callbacks_are_independent(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	nv12_result = p010_result = true;
	graphics.exports.device_nv12_available = NULL;
	assert_false(gs_nv12_available());
	assert_true(gs_p010_available());
	assert_int_equal(nv12_calls, 0);
	assert_int_equal(p010_calls, 1);
	graphics.exports.device_nv12_available = nv12;
	graphics.exports.device_p010_available = NULL;
	assert_false(gs_p010_available());
	assert_true(gs_nv12_available());
	assert_int_equal(nv12_calls, 1);
	assert_int_equal(p010_calls, 1);
	graphics.exports.device_nv12_available = NULL;
	assert_false(gs_nv12_available());
	assert_false(gs_p010_available());
	assert_int_equal(nv12_calls, 1);
	assert_int_equal(p010_calls, 1);
	assert_int_equal(log_count, 0);
}

static void callback_results_and_device_routing(void **state)
{
	(void)state;
	/* Every device and result here is fake; host capabilities are irrelevant. */
	for (unsigned device = 0; device < 2; device++) {
		expected_device = (gs_device_t *)&device_tokens[device];
		graphics.device = expected_device;
		gs_enter_context(&graphics);
		for (unsigned results = 0; results < 4; results++) {
			nv12_result = (results & 1) != 0;
			p010_result = (results & 2) != 0;
			unsigned calls = device * 4 + results;
			assert_int_equal(gs_nv12_available(), nv12_result);
			assert_int_equal(nv12_calls, calls + 1);
			assert_int_equal(p010_calls, calls);
			assert_int_equal(gs_p010_available(), p010_result);
			assert_int_equal(nv12_calls, calls + 1);
			assert_int_equal(p010_calls, calls + 1);
			assert_int_equal(log_count, 0);
		}
		gs_leave_context();
		assert_null(gs_get_context());
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_context, setup, teardown),
		cmocka_unit_test_setup_teardown(optional_callbacks_are_independent, setup, teardown),
		cmocka_unit_test_setup_teardown(callback_results_and_device_routing, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
