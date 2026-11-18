/* Characterize public device queries against actual libobs without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

/* The characterization lands with the extraction, before the Rust swap. */
#if !__has_include("graphics/graphics-device-queries.c.h")
#error "graphics device-query extraction seam is missing"
#endif

static struct graphics_subsystem graphics;
static int device_token, device_type;
static uint32_t adapter_count;
static unsigned type_calls, adapter_calls, log_count;
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
	assert_ptr_equal(device, &device_token);
}

static int query_type(void)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	type_calls++;
	return device_type;
}

static uint32_t query_adapters(void)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	adapter_calls++;
	return adapter_count;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	type_calls = adapter_calls = log_count = 0;
	message[0] = '\0';
	device_type = 42;
	adapter_count = 37;
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_get_type = query_type;
	graphics.exports.gs_get_adapter_count = query_adapters;
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
	/* With callbacks present or absent, validate context before dispatch. */
	for (unsigned i = 0; i < 2; i++) {
		assert_int_equal(gs_get_device_type(), -1);
		assert_string_equal(message, "gs_get_device_type: called while not in a graphics context");
		assert_int_equal(log_count, 2 * i + 1);
		assert_int_equal(gs_get_adapter_count(), 0);
		assert_string_equal(message, "gs_get_adapter_count: called while not in a graphics context");
		assert_int_equal(log_count, 2 * i + 2);
		assert_int_equal(type_calls, 0);
		assert_int_equal(adapter_calls, 0);
		graphics.exports.device_get_type = NULL;
		graphics.exports.gs_get_adapter_count = NULL;
	}
}

static void optional_adapter_callback(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	graphics.exports.gs_get_adapter_count = NULL;
	/* Characterized, not endorsed: absent and zero-valued backends both return zero. */
	assert_int_equal(gs_get_adapter_count(), 0);
	assert_int_equal(adapter_calls, 0);
	assert_int_equal(type_calls, 0);
	assert_int_equal(log_count, 0);
	assert_string_equal(message, "");
	graphics.exports.gs_get_adapter_count = query_adapters;
	adapter_count = 0;
	assert_int_equal(gs_get_adapter_count(), 0);
	assert_int_equal(adapter_calls, 1);
	assert_int_equal(log_count, 0);
	graphics.exports.gs_get_adapter_count = NULL;
	assert_int_equal(gs_get_adapter_count(), 0);
	assert_int_equal(adapter_calls, 1);
	assert_int_equal(type_calls, 0);
	assert_int_equal(log_count, 0);
}

static void device_type_values(void **state)
{
	(void)state;
	const int values[] = {INT_MIN, -7, -1, 0, 1, 2, 42, INT_MAX};
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		device_type = values[i];
		assert_int_equal(gs_get_device_type(), values[i]);
		assert_int_equal(type_calls, i + 1);
		assert_int_equal(adapter_calls, 0);
		assert_int_equal(log_count, 0);
	}
	assert_string_equal(message, "");
}

static void adapter_count_values(void **state)
{
	(void)state;
	const uint32_t values[] = {0, 1, 2, 37, UINT32_C(0x80000000), UINT32_MAX};
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		adapter_count = values[i];
		assert_int_equal(gs_get_adapter_count(), values[i]);
		assert_int_equal(adapter_calls, i + 1);
		assert_int_equal(type_calls, 0);
		assert_int_equal(log_count, 0);
	}
	assert_string_equal(message, "");
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(no_context, setup, teardown),
					   cmocka_unit_test_setup_teardown(optional_adapter_callback, setup, teardown),
					   cmocka_unit_test_setup_teardown(device_type_values, setup, teardown),
					   cmocka_unit_test_setup_teardown(adapter_count_values, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
