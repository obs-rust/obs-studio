/* Characterize borrowed shader matrix-parameter queries without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

/* The characterization lands with the extraction, before the Rust swap. */
#if !__has_include("graphics/graphics-shader-matrix-queries.c.h")
#error "graphics shader matrix-parameter query extraction seam is missing"
#endif

static struct graphics_subsystem graphics;
static int device_token, shader_tokens[2], parameter_tokens[2];
static const gs_shader_t *expected_shader;
static gs_sparam_t *viewproj_result, *world_result;
static unsigned viewproj_calls, world_calls, log_count;
static char message[160], events[32];
static size_t event_count;
static log_handler_t previous_handler;
static void *previous_param;

static void record(char event)
{
	assert_true(event_count + 1 < sizeof(events));
	events[event_count++] = event;
	events[event_count] = '\0';
}

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture(int level, const char *format, va_list args, void *param)
{
	(void)param;
	assert_int_equal(level, LOG_DEBUG);
	log_count++;
	vsnprintf(message, sizeof(message), format, args);
	record('L');
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static gs_sparam_t *viewproj(const gs_shader_t *shader)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(shader, expected_shader);
	viewproj_calls++;
	record('V');
	return viewproj_result;
}

static gs_sparam_t *world(const gs_shader_t *shader)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(shader, expected_shader);
	world_calls++;
	record('W');
	return world_result;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	viewproj_calls = world_calls = log_count = 0;
	event_count = 0;
	message[0] = events[0] = '\0';
	expected_shader = NULL;
	viewproj_result = world_result = NULL;
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.gs_shader_get_viewproj_matrix = viewproj;
	graphics.exports.gs_shader_get_world_matrix = world;
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
	/* Context rejection precedes shader rejection, even without callbacks. */
	for (unsigned i = 0; i < 2; i++) {
		const gs_shader_t *shader = i ? NULL : (const gs_shader_t *)&shader_tokens[0];
		assert_null(gs_shader_get_viewproj_matrix(shader));
		assert_string_equal(message, "gs_shader_get_viewproj_matrix: called while not in a graphics context");
		assert_int_equal(log_count, 2 * i + 1);
		assert_null(gs_shader_get_world_matrix(shader));
		assert_string_equal(message, "gs_shader_get_world_matrix: called while not in a graphics context");
		assert_int_equal(log_count, 2 * i + 2);
		assert_int_equal(viewproj_calls, 0);
		assert_int_equal(world_calls, 0);
		graphics.exports.gs_shader_get_viewproj_matrix = NULL;
		graphics.exports.gs_shader_get_world_matrix = NULL;
	}
	assert_string_equal(events, "LLLL");
}

static void null_shader_in_context(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < 2; i++) {
		assert_null(gs_shader_get_viewproj_matrix(NULL));
		assert_string_equal(message, "gs_shader_get_viewproj_matrix: Null 'shader' parameter");
		assert_int_equal(log_count, 2 * i + 1);
		assert_null(gs_shader_get_world_matrix(NULL));
		assert_string_equal(message, "gs_shader_get_world_matrix: Null 'shader' parameter");
		assert_int_equal(log_count, 2 * i + 2);
		assert_int_equal(viewproj_calls, 0);
		assert_int_equal(world_calls, 0);
		graphics.exports.gs_shader_get_viewproj_matrix = NULL;
		graphics.exports.gs_shader_get_world_matrix = NULL;
	}
	assert_string_equal(events, "LLLL");
}

static void queries_forward_shader_and_borrowed_results(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	viewproj_result = (gs_sparam_t *)&parameter_tokens[0];
	world_result = (gs_sparam_t *)&parameter_tokens[1];
	for (unsigned i = 0; i < 2; i++) {
		expected_shader = (const gs_shader_t *)&shader_tokens[i];
		assert_ptr_equal(gs_shader_get_viewproj_matrix(expected_shader), viewproj_result);
		assert_int_equal(viewproj_calls, i + 1);
		assert_int_equal(world_calls, i);
		assert_ptr_equal(gs_shader_get_world_matrix(expected_shader), world_result);
		assert_int_equal(viewproj_calls, i + 1);
		assert_int_equal(world_calls, i + 1);
		assert_int_equal(log_count, 0);
	}
	assert_string_equal(events, "VWVW");
}

static void callback_null_results_are_returned_silently(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	expected_shader = (const gs_shader_t *)&shader_tokens[0];
	assert_null(gs_shader_get_world_matrix(expected_shader));
	assert_int_equal(world_calls, 1);
	assert_int_equal(viewproj_calls, 0);
	assert_null(gs_shader_get_viewproj_matrix(expected_shader));
	assert_int_equal(world_calls, 1);
	assert_int_equal(viewproj_calls, 1);
	assert_int_equal(log_count, 0);
	assert_string_equal(events, "WV");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_context, setup, teardown),
		cmocka_unit_test_setup_teardown(null_shader_in_context, setup, teardown),
		cmocka_unit_test_setup_teardown(queries_forward_shader_and_borrowed_results, setup, teardown),
		cmocka_unit_test_setup_teardown(callback_null_results_are_returned_silently, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
