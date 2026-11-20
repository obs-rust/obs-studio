/* Characterize borrowed device identity through actual libobs, without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

/* Keep this characterization unchanged when the standalone C unit is swapped. */
#if !__has_include("graphics/graphics-device-identity.c.h")
#error "graphics device-identity extraction seam is missing"
#endif

static struct graphics_subsystem graphics;
static int device_token, other_device_token, object_tokens[2];
static void *object_result;
static const char *name_result;
static unsigned object_calls, name_calls, log_count;
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

static void *query_object(gs_device_t *device)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(device, graphics.device);
	object_calls++;
	return object_result;
}

static const char *query_name(void)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	name_calls++;
	return name_result;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	object_calls = name_calls = log_count = 0;
	object_result = NULL;
	name_result = NULL;
	message[0] = '\0';
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_get_device_obj = query_object;
	graphics.exports.device_get_name = query_name;
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
	/* Admission must precede export-table access, even with absent callbacks. */
	for (unsigned i = 0; i < 2; i++) {
		assert_null(gs_get_device_obj());
		assert_string_equal(message, "gs_get_device_obj: called while not in a graphics context");
		assert_int_equal(log_count, 2 * i + 1);
		assert_null(gs_get_device_name());
		assert_string_equal(message, "gs_get_device_name: called while not in a graphics context");
		assert_int_equal(log_count, 2 * i + 2);
		assert_int_equal(object_calls, 0);
		assert_int_equal(name_calls, 0);
		graphics.exports.device_get_device_obj = NULL;
		graphics.exports.device_get_name = NULL;
	}
}

static void borrowed_object(void **state)
{
	(void)state;
	void *const values[] = {NULL, &object_tokens[0], &object_tokens[1], &object_tokens[0]};
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		object_result = values[i];
		assert_ptr_equal(gs_get_device_obj(), values[i]);
		assert_int_equal(object_calls, i + 1);
		assert_int_equal(name_calls, 0);
		assert_int_equal(log_count, 0);
	}
	/* The admitted wrapper forwards the current native device, even NULL. */
	graphics.device = (gs_device_t *)&other_device_token;
	assert_ptr_equal(gs_get_device_obj(), object_result);
	graphics.device = NULL;
	assert_ptr_equal(gs_get_device_obj(), object_result);
	assert_int_equal(object_calls, 6);
	assert_int_equal(name_calls, 0);
	assert_int_equal(log_count, 0);
	graphics.device = (gs_device_t *)&device_token;
}

static void borrowed_name(void **state)
{
	(void)state;
	char empty[] = "", first[] = "recording backend", same_bytes[] = "recording backend";
	const char *const values[] = {NULL, empty, first, same_bytes, first};
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		name_result = values[i];
		assert_ptr_equal(gs_get_device_name(), values[i]);
		assert_int_equal(name_calls, i + 1);
		assert_int_equal(object_calls, 0);
		assert_int_equal(log_count, 0);
	}
	const char *borrowed = gs_get_device_name();
	first[0] = 'R';
	assert_string_equal(borrowed, "Recording backend");
	assert_string_equal(same_bytes, "recording backend");
	/* Name dispatch has no device argument and needs only the admitted context. */
	graphics.device = NULL;
	assert_ptr_equal(gs_get_device_name(), first);
	assert_int_equal(name_calls, 7);
	assert_int_equal(object_calls, 0);
	assert_int_equal(log_count, 0);
	graphics.device = (gs_device_t *)&device_token;
}

static void context_leave_reenter(void **state)
{
	(void)state;
	object_result = &object_tokens[0];
	name_result = "backend";
	gs_enter_context(&graphics);
	assert_ptr_equal(gs_get_device_obj(), object_result);
	assert_ptr_equal(gs_get_device_name(), name_result);
	gs_leave_context();
	assert_null(gs_get_context());
	assert_null(gs_get_device_obj());
	assert_string_equal(message, "gs_get_device_obj: called while not in a graphics context");
	assert_null(gs_get_device_name());
	assert_string_equal(message, "gs_get_device_name: called while not in a graphics context");
	assert_int_equal(log_count, 2);
	assert_int_equal(object_calls, 1);
	assert_int_equal(name_calls, 1);
	gs_enter_context(&graphics);
	assert_ptr_equal(gs_get_device_obj(), object_result);
	assert_ptr_equal(gs_get_device_name(), name_result);
	assert_int_equal(object_calls, 2);
	assert_int_equal(name_calls, 2);
	assert_int_equal(log_count, 2);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(no_context, setup, teardown),
					   cmocka_unit_test_setup_teardown(borrowed_object, setup, teardown),
					   cmocka_unit_test_setup_teardown(borrowed_name, setup, teardown),
					   cmocka_unit_test_setup_teardown(context_leave_reenter, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
