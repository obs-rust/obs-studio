/* Characterization of the actual libobs setter (#4141), without a GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>

#include "graphics/graphics-internal.h"
#include "util/base.h"

static unsigned log_count;
static int log_level;
static char log_text[128];

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_count++;
	log_level = level;
	vsnprintf(log_text, sizeof(log_text), format, args);
}

static void device_context(gs_device_t *device)
{
	(void)device;
}

static void setter(void **state)
{
	(void)state;
	struct graphics_subsystem graphics = {0};
	graphics.exports.device_enter_context = device_context;
	graphics.exports.device_leave_context = device_context;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_set_log_handler(capture_log, NULL);
	assert_null(gs_get_context());
	assert_false(gs_set_linear_srgb(true));
	assert_int_equal(log_count, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_text, "gs_set_linear_srgb: called while not in a graphics context");
	gs_enter_context(&graphics);
	assert_ptr_equal(gs_get_context(), &graphics);
	for (unsigned previous = 0; previous < 2; previous++) {
		for (unsigned next = 0; next < 2; next++) {
			graphics.linear_srgb = previous != 0;
			assert_int_equal(gs_set_linear_srgb(next != 0), previous != 0);
			assert_int_equal(graphics.linear_srgb, next != 0);
			assert_int_equal(gs_set_linear_srgb(next != 0), next != 0);
			assert_int_equal(gs_get_linear_srgb(), next != 0);
		}
	}
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_int_equal(graphics.ref, 1);
	assert_int_equal(log_count, 1);
	gs_leave_context();
	assert_null(gs_get_context());
	assert_false(gs_set_linear_srgb(false));
	assert_int_equal(log_count, 2);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_text, "gs_set_linear_srgb: called while not in a graphics context");
	base_set_log_handler(NULL, NULL);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(setter)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
