/* Characterize borrowed active shader queries without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

static struct graphics_subsystem graphics;
static int device_tokens[2];
static int shader_tokens[2];
static gs_device_t *expected_device;
static gs_shader_t *vertex_result, *pixel_result;
static unsigned vertex_calls, pixel_calls, log_count;
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

static gs_shader_t *vertex(const gs_device_t *device)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(device, expected_device);
	vertex_calls++;
	return vertex_result;
}

static gs_shader_t *pixel(const gs_device_t *device)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(device, expected_device);
	pixel_calls++;
	return pixel_result;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	vertex_calls = pixel_calls = log_count = 0;
	vertex_result = pixel_result = NULL;
	shader_tokens[0] = 17;
	shader_tokens[1] = 29;
	message[0] = '\0';
	expected_device = (gs_device_t *)&device_tokens[0];
	graphics.device = expected_device;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_get_vertex_shader = vertex;
	graphics.exports.device_get_pixel_shader = pixel;
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
	/* A NULL graphics handle never establishes a thread context. */
	gs_enter_context(NULL);
	assert_string_equal(message, "gs_enter_context: Null 'graphics' parameter");
	assert_int_equal(log_count, 1);
	assert_null(gs_get_context());
	for (unsigned i = 0; i < 2; i++) {
		assert_null(gs_get_vertex_shader());
		assert_string_equal(message, "gs_get_vertex_shader: called while not in a graphics context");
		assert_int_equal(log_count, 2 * i + 2);
		assert_null(gs_get_pixel_shader());
		assert_string_equal(message, "gs_get_pixel_shader: called while not in a graphics context");
		assert_int_equal(log_count, 2 * i + 3);
		assert_int_equal(vertex_calls, 0);
		assert_int_equal(pixel_calls, 0);
		/* Rejection precedes reading either mandatory backend callback. */
		graphics.exports.device_get_vertex_shader = NULL;
		graphics.exports.device_get_pixel_shader = NULL;
	}
}

static void callback_results_and_device_routing(void **state)
{
	(void)state;
	/* Opaque tokens are borrowed identities, never real host shaders/devices. */
	for (unsigned device = 0; device < 2; device++) {
		expected_device = (gs_device_t *)&device_tokens[device];
		graphics.device = expected_device;
		gs_enter_context(&graphics);
		for (unsigned results = 0; results < 4; results++) {
			vertex_result = (results & 1) ? (gs_shader_t *)&shader_tokens[device] : NULL;
			pixel_result = (results & 2) ? (gs_shader_t *)&shader_tokens[1 - device] : NULL;
			unsigned calls = device * 4 + results;
			assert_ptr_equal(gs_get_vertex_shader(), vertex_result);
			assert_int_equal(vertex_calls, calls + 1);
			assert_int_equal(pixel_calls, calls);
			assert_ptr_equal(gs_get_pixel_shader(), pixel_result);
			assert_int_equal(vertex_calls, calls + 1);
			assert_int_equal(pixel_calls, calls + 1);
			assert_int_equal(graphics.ref, 1);
			assert_int_equal(shader_tokens[0], 17);
			assert_int_equal(shader_tokens[1], 29);
			assert_int_equal(log_count, 0);
		}
		gs_leave_context();
		assert_null(gs_get_context());
	}
}

static void leaving_context_rejects_then_reentry_dispatches(void **state)
{
	(void)state;
	vertex_result = (gs_shader_t *)&shader_tokens[0];
	pixel_result = (gs_shader_t *)&shader_tokens[1];
	gs_enter_context(&graphics);
	assert_ptr_equal(gs_get_pixel_shader(), pixel_result);
	assert_ptr_equal(gs_get_vertex_shader(), vertex_result);
	gs_leave_context();
	assert_null(gs_get_vertex_shader());
	assert_string_equal(message, "gs_get_vertex_shader: called while not in a graphics context");
	assert_null(gs_get_pixel_shader());
	assert_string_equal(message, "gs_get_pixel_shader: called while not in a graphics context");
	assert_int_equal(log_count, 2);
	assert_int_equal(vertex_calls, 1);
	assert_int_equal(pixel_calls, 1);
	gs_enter_context(&graphics);
	assert_ptr_equal(gs_get_vertex_shader(), vertex_result);
	assert_ptr_equal(gs_get_pixel_shader(), pixel_result);
	assert_int_equal(vertex_calls, 2);
	assert_int_equal(pixel_calls, 2);
	assert_int_equal(log_count, 2);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_context, setup, teardown),
		cmocka_unit_test_setup_teardown(callback_results_and_device_routing, setup, teardown),
		cmocka_unit_test_setup_teardown(leaving_context_rejects_then_reentry_dispatches, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
