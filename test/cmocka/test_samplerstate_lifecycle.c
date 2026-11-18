/* Characterize sampler-state lifecycle dispatch against libobs without a GPU. */
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
#if !__has_include("graphics/graphics-samplerstate-lifecycle.c.h")
#error "graphics sampler-state lifecycle extraction seam is missing"
#endif

static struct graphics_subsystem graphics;
static int device_token, sampler_tokens[2];
static const struct gs_sampler_info *expected_info;
static gs_samplerstate_t *result, *expected_sampler;
static unsigned create_calls, destroy_calls, log_count;
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

static gs_samplerstate_t *create(gs_device_t *device, const struct gs_sampler_info *info)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(device, &device_token);
	assert_ptr_equal(info, expected_info);
	create_calls++;
	record('C');
	return result;
}

static void destroy(gs_samplerstate_t *sampler)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(sampler, expected_sampler);
	destroy_calls++;
	record('D');
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	create_calls = destroy_calls = log_count = 0;
	event_count = 0;
	message[0] = events[0] = '\0';
	expected_info = NULL;
	result = expected_sampler = NULL;
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_samplerstate_create = create;
	graphics.exports.gs_samplerstate_destroy = destroy;
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
	struct gs_sampler_info info = {0};
	/* Context is validated even for NULL arguments and absent callbacks. */
	for (unsigned i = 0; i < 2; i++) {
		assert_null(gs_samplerstate_create(&info));
		assert_string_equal(message, "gs_samplerstate_create: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 1);
		assert_null(gs_samplerstate_create(NULL));
		assert_string_equal(message, "gs_samplerstate_create: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 2);
		gs_samplerstate_destroy((gs_samplerstate_t *)&sampler_tokens[0]);
		assert_string_equal(message, "gs_samplerstate_destroy: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 3);
		gs_samplerstate_destroy(NULL);
		assert_string_equal(message, "gs_samplerstate_destroy: called while not in a graphics context");
		assert_int_equal(log_count, 4 * i + 4);
		assert_int_equal(create_calls, 0);
		assert_int_equal(destroy_calls, 0);
		graphics.exports.device_samplerstate_create = NULL;
		graphics.exports.gs_samplerstate_destroy = NULL;
	}
	assert_string_equal(events, "LLLLLLLL");
}

static void null_arguments_in_context(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < 2; i++) {
		assert_null(gs_samplerstate_create(NULL));
		assert_string_equal(message, "gs_samplerstate_create: Null 'info' parameter");
		assert_int_equal(log_count, i + 1);
		/* Characterized, not endorsed: NULL destroy still requires context,
		 * but is silent once that context exists, even without a callback. */
		gs_samplerstate_destroy(NULL);
		assert_int_equal(log_count, i + 1);
		assert_int_equal(create_calls, 0);
		assert_int_equal(destroy_calls, 0);
		graphics.exports.device_samplerstate_create = NULL;
		graphics.exports.gs_samplerstate_destroy = NULL;
	}
	assert_string_equal(events, "LL");
}

static void creation_forwards_info_and_result(void **state)
{
	(void)state;
	struct gs_sampler_info infos[2] = {
		{GS_FILTER_POINT, GS_ADDRESS_CLAMP, GS_ADDRESS_WRAP, GS_ADDRESS_MIRROR, INT_MIN, 0},
		{GS_FILTER_ANISOTROPIC, GS_ADDRESS_BORDER, GS_ADDRESS_MIRRORONCE, GS_ADDRESS_CLAMP, INT_MAX,
		 UINT32_MAX},
	};
	struct gs_sampler_info before[2];
	memcpy(before, infos, sizeof(infos));
	gs_enter_context(&graphics);
	/* The wrapper forwards opaque returns, including backend failure (NULL),
	 * and neither copies nor normalizes the caller's settings. */
	for (unsigned i = 0; i < 3; i++) {
		expected_info = &infos[i % 2];
		result = i < 2 ? (gs_samplerstate_t *)&sampler_tokens[i] : NULL;
		assert_ptr_equal(gs_samplerstate_create(expected_info), result);
		assert_int_equal(create_calls, i + 1);
		assert_int_equal(destroy_calls, 0);
		assert_int_equal(log_count, 0);
		assert_memory_equal(infos, before, sizeof(infos));
	}
	assert_string_equal(events, "CCC");
}

static void destruction_forwards_handles_in_order(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < 3; i++) {
		expected_sampler = (gs_samplerstate_t *)&sampler_tokens[i % 2];
		/* A recording backend owns no allocation: even repeated handles are
		 * forwarded once per call, with no wrapper-side lifecycle tracking. */
		gs_samplerstate_destroy(expected_sampler);
		assert_int_equal(destroy_calls, i + 1);
		gs_samplerstate_destroy(NULL);
		assert_int_equal(destroy_calls, i + 1);
		assert_int_equal(create_calls, 0);
		assert_int_equal(log_count, 0);
	}
	assert_string_equal(events, "DDD");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_context, setup, teardown),
		cmocka_unit_test_setup_teardown(null_arguments_in_context, setup, teardown),
		cmocka_unit_test_setup_teardown(creation_forwards_info_and_result, setup, teardown),
		cmocka_unit_test_setup_teardown(destruction_forwards_handles_in_order, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
