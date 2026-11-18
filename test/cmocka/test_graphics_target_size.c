/* Characterize target-size dispatch against actual libobs without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

/* The characterization lands with the extraction, before the Rust swap. */
#if !__has_include("graphics/graphics-target-size.c.h")
#error "graphics target-size extraction seam is missing"
#endif

static struct graphics_subsystem graphics;
static int device_token;
static uint32_t width, height;
static uint32_t *expected_x, *expected_y;
static unsigned size_calls, width_calls, height_calls, log_count;
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

static void query_size(const gs_device_t *device, uint32_t *x, uint32_t *y)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(device, &device_token);
	assert_ptr_equal(x, expected_x);
	assert_ptr_equal(y, expected_y);
	size_calls++;
	/* Tolerant fake: record nullable outputs without dereferencing them. */
	if (x)
		*x = width;
	if (y)
		*y = height;
}

static uint32_t query_width(const gs_device_t *device)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(device, &device_token);
	width_calls++;
	return width;
}

static uint32_t query_height(const gs_device_t *device)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(device, &device_token);
	height_calls++;
	return height;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	size_calls = width_calls = height_calls = log_count = 0;
	message[0] = '\0';
	width = 1920;
	height = 1080;
	expected_x = expected_y = NULL;
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_get_size = query_size;
	graphics.exports.device_get_width = query_width;
	graphics.exports.device_get_height = query_height;
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
	uint32_t x = UINT32_C(0xa5a5a5a5), y = UINT32_C(0x5a5a5a5a);
	/* Validate context before dispatch, with callbacks present or absent. */
	for (unsigned i = 0; i < 2; i++) {
		gs_get_size(&x, &y);
		assert_string_equal(message, "gs_get_size: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 1);
		assert_int_equal(x, UINT32_C(0xa5a5a5a5));
		assert_int_equal(y, UINT32_C(0x5a5a5a5a));
		gs_get_size(NULL, NULL);
		assert_string_equal(message, "gs_get_size: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 2);
		assert_int_equal(gs_get_width(), 0);
		assert_string_equal(message, "gs_get_width: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 3);
		assert_int_equal(gs_get_height(), 0);
		assert_string_equal(message, "gs_get_height: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 4);
		assert_int_equal(size_calls, 0);
		assert_int_equal(width_calls, 0);
		assert_int_equal(height_calls, 0);
		graphics.exports.device_get_size = NULL;
		graphics.exports.device_get_width = NULL;
		graphics.exports.device_get_height = NULL;
	}
}

static void size_values(void **state)
{
	(void)state;
	const uint32_t values[][2] = {{1920, 1080},
				      {0, 0},
				      {UINT32_C(0x80000000), UINT32_MAX},
				      {UINT32_MAX, UINT32_C(0x80000000)}};
	uint32_t x, y;
	expected_x = &x;
	expected_y = &y;
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		width = values[i][0];
		height = values[i][1];
		x = y = 17;
		gs_get_size(&x, &y);
		assert_int_equal(x, width);
		assert_int_equal(y, height);
		assert_int_equal(size_calls, i + 1);
		assert_int_equal(width_calls, 0);
		assert_int_equal(height_calls, 0);
		assert_int_equal(log_count, 0);
	}
}

static void nullable_and_aliased_outputs(void **state)
{
	(void)state;
	uint32_t value = 17;
	gs_enter_context(&graphics);
	/* Characterized, not endorsed: the public API adds no output-pointer guards. */
	gs_get_size(NULL, NULL);
	assert_int_equal(size_calls, 1);
	expected_x = &value;
	gs_get_size(&value, NULL);
	assert_int_equal(value, width);
	assert_int_equal(size_calls, 2);
	expected_x = NULL;
	expected_y = &value;
	gs_get_size(NULL, &value);
	assert_int_equal(value, height);
	assert_int_equal(size_calls, 3);
	expected_x = &value;
	gs_get_size(&value, &value);
	assert_int_equal(value, height);
	assert_int_equal(size_calls, 4);
	assert_int_equal(width_calls, 0);
	assert_int_equal(height_calls, 0);
	assert_int_equal(log_count, 0);
}

static void scalar_values(void **state)
{
	(void)state;
	const uint32_t values[] = {1920, 1080, 0, 1, UINT32_C(0x80000000), UINT32_MAX};
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		width = values[i];
		height = values[sizeof(values) / sizeof(values[0]) - i - 1];
		assert_int_equal(gs_get_width(), width);
		assert_int_equal(width_calls, i + 1);
		assert_int_equal(height_calls, i);
		assert_int_equal(gs_get_height(), height);
		assert_int_equal(height_calls, i + 1);
		assert_int_equal(width_calls, i + 1);
		assert_int_equal(size_calls, 0);
		assert_int_equal(log_count, 0);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(no_context, setup, teardown),
					   cmocka_unit_test_setup_teardown(size_values, setup, teardown),
					   cmocka_unit_test_setup_teardown(nullable_and_aliased_outputs, setup,
									   teardown),
					   cmocka_unit_test_setup_teardown(scalar_values, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
