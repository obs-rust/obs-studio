/* Characterize the public C projection helper before its Rust swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

#include "graphics/graphics-internal.h"
#include "util/base.h"

static gs_device_t *seen_device;
static float seen_args[6];
static unsigned calls, logs;
static int log_level;
static char log_text[128];
static int device_tokens[2];

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	logs++;
	log_level = level;
	vsnprintf(log_text, sizeof(log_text), format, args);
}

static void device_context(gs_device_t *device)
{
	assert_true(device == (gs_device_t *)&device_tokens[0] || device == (gs_device_t *)&device_tokens[1]);
}

static void capture_frustum(gs_device_t *device, float left, float right, float top, float bottom, float znear,
			    float zfar)
{
	calls++;
	seen_device = device;
	seen_args[0] = left;
	seen_args[1] = right;
	seen_args[2] = top;
	seen_args[3] = bottom;
	seen_args[4] = znear;
	seen_args[5] = zfar;
}

static void perspective_contract(void **state)
{
	(void)state;
	log_handler_t old_handler;
	void *old_param;
	base_get_log_handler(&old_handler, &old_param);
	base_set_log_handler(capture_log, NULL);
	assert_null(gs_get_context());
	gs_perspective(60.0f, 1.5f, 0.1f, 100.0f);
	assert_int_equal(calls, 0);
	assert_int_equal(logs, 1);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_text, "gs_perspective: called while not in a graphics context");

	const float cases[][4] = {
		{60.0f, 16.0f / 9.0f, 0.1f, 1000.0f},
		{45.0f, 4.0f / 3.0f, 1.0f, 100.0f},
		{93.125f, 0.73125f, 0.03125f, 123.456f},
		{0.001f, 2.125f, 0.0001f, 100000.0f},
		{179.0f, 1.0f, 2.0f, 3.0f},
		/* Characterized, not endorsed: no range checks, even on the clips. */
		{-35.5f, -1.25f, -0.75f, -20.0f},
		{80.0f, -0.0f, 10.0f, 1.0f},
		{0.0f, 1.0f, 1.0f, -0.0f},
		{-0.0f, 1.0f, 1.0f, 0.0f},
		{72.0f, 1.5f, -0.0f, 0.0f},
	};
	for (size_t device = 0; device < 2; device++) {
		struct graphics_subsystem graphics = {0};
		graphics.device = (gs_device_t *)&device_tokens[device];
		graphics.exports.device_enter_context = device_context;
		graphics.exports.device_leave_context = device_context;
		graphics.exports.device_frustum = capture_frustum;
		assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
		gs_enter_context(&graphics);
		for (size_t i = 0; i < sizeof(cases) / sizeof(cases[0]); i++) {
			/* Force runtime tanf, as in libobs, rather than constant folding.
			 * Keep the original float RAD multiplication and half-angle order. */
			volatile float angle = cases[i][0];
			float ymax = cases[i][2] * tanf((angle * 0.0174532925199432957692369076848f) * 0.5f);
			float ymin = -ymax;
			float expected[] = {ymin * cases[i][1], ymax * cases[i][1], ymin, ymax,
					    cases[i][2],        cases[i][3]};
			unsigned previous = calls;
			gs_perspective(cases[i][0], cases[i][1], cases[i][2], cases[i][3]);
			assert_int_equal(calls, previous + 1);
			assert_ptr_equal(seen_device, graphics.device);
			for (size_t arg = 0; arg < 6; arg++)
				assert_memory_equal(&seen_args[arg], &expected[arg], sizeof(float));
			assert_int_equal(logs, 1);
		}
		gs_leave_context();
		assert_null(gs_get_context());
		assert_int_equal(graphics.ref, 0);
		assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	}
	unsigned previous = calls;
	gs_perspective(45.0f, 1.0f, 1.0f, 10.0f);
	assert_int_equal(calls, previous);
	assert_int_equal(logs, 2);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_text, "gs_perspective: called while not in a graphics context");
	base_set_log_handler(old_handler, old_param);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(perspective_contract)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
