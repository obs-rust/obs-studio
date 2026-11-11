/* Characterize the public libobs stage-surface getters without a GPU (#4159). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "graphics/graphics-internal.h"
#include "util/base.h"

static unsigned log_count;
static int log_level;
static char log_text[128];
static unsigned calls[3];
static const gs_stagesurf_t *seen_surface;
static uint32_t width_result, height_result;
static enum gs_color_format format_result;

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

static uint32_t backend_width(const gs_stagesurf_t *surface)
{
	calls[0]++;
	seen_surface = surface;
	return width_result;
}

static uint32_t backend_height(const gs_stagesurf_t *surface)
{
	calls[1]++;
	seen_surface = surface;
	return height_result;
}

static enum gs_color_format backend_format(const gs_stagesurf_t *surface)
{
	calls[2]++;
	seen_surface = surface;
	return format_result;
}

static void assert_diagnostic(const char *function, const char *reason, unsigned count)
{
	char expected[128];
	snprintf(expected, sizeof(expected), "%s: %s", function, reason);
	assert_int_equal(log_count, count);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_text, expected);
}

static void getters(void **state)
{
	(void)state;
	struct graphics_subsystem graphics = {0};
	/* Callbacks only compare this opaque token; no surface storage is read. */
	const gs_stagesurf_t *surface = (const gs_stagesurf_t *)&graphics;
	const char *names[] = {"gs_stagesurface_get_width", "gs_stagesurface_get_height",
			       "gs_stagesurface_get_color_format"};
	graphics.exports.device_enter_context = device_context;
	graphics.exports.device_leave_context = device_context;
	graphics.exports.gs_stagesurface_get_width = backend_width;
	graphics.exports.gs_stagesurface_get_height = backend_height;
	graphics.exports.gs_stagesurface_get_color_format = backend_format;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_set_log_handler(capture_log, NULL);
	assert_null(gs_get_context());

	/* Context validation precedes surface validation, even for NULL. */
	assert_int_equal(gs_stagesurface_get_width(NULL), 0);
	assert_diagnostic(names[0], "called while not in a graphics context", 1);
	assert_int_equal(gs_stagesurface_get_height(NULL), 0);
	assert_diagnostic(names[1], "called while not in a graphics context", 2);
	assert_int_equal(gs_stagesurface_get_color_format(NULL), GS_UNKNOWN);
	assert_diagnostic(names[2], "called while not in a graphics context", 3);
	assert_int_equal(gs_stagesurface_get_width(surface), 0);
	assert_diagnostic(names[0], "called while not in a graphics context", 4);
	assert_int_equal(gs_stagesurface_get_height(surface), 0);
	assert_diagnostic(names[1], "called while not in a graphics context", 5);
	assert_int_equal(gs_stagesurface_get_color_format(surface), GS_UNKNOWN);
	assert_diagnostic(names[2], "called while not in a graphics context", 6);

	gs_enter_context(&graphics);
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_int_equal(gs_stagesurface_get_width(NULL), 0);
	assert_diagnostic(names[0], "Null 'stagesurf' parameter", 7);
	assert_int_equal(gs_stagesurface_get_height(NULL), 0);
	assert_diagnostic(names[1], "Null 'stagesurf' parameter", 8);
	assert_int_equal(gs_stagesurface_get_color_format(NULL), GS_UNKNOWN);
	assert_diagnostic(names[2], "Null 'stagesurf' parameter", 9);
	assert_int_equal(calls[0] + calls[1] + calls[2], 0);
	assert_null(seen_surface);

	const uint32_t sizes[] = {0, 1, 1920, UINT32_MAX};
	const enum gs_color_format formats[] = {GS_UNKNOWN, GS_RGBA, GS_RGBA16F, GS_R8};
	for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++) {
		width_result = sizes[i];
		height_result = sizes[3 - i];
		format_result = formats[i];
		seen_surface = NULL;
		assert_int_equal(gs_stagesurface_get_width(surface), width_result);
		assert_ptr_equal(seen_surface, surface);
		assert_int_equal(calls[0], i + 1);
		assert_int_equal(calls[1], i);
		assert_int_equal(calls[2], i);
		seen_surface = NULL;
		assert_int_equal(gs_stagesurface_get_height(surface), height_result);
		assert_ptr_equal(seen_surface, surface);
		assert_int_equal(calls[0], i + 1);
		assert_int_equal(calls[1], i + 1);
		assert_int_equal(calls[2], i);
		seen_surface = NULL;
		assert_int_equal(gs_stagesurface_get_color_format(surface), format_result);
		assert_ptr_equal(seen_surface, surface);
		assert_int_equal(calls[0], i + 1);
		assert_int_equal(calls[1], i + 1);
		assert_int_equal(calls[2], i + 1);
		assert_ptr_equal(gs_get_context(), &graphics);
		assert_int_equal(graphics.ref, 1);
		assert_int_equal(log_count, 9);
	}

	gs_leave_context();
	assert_null(gs_get_context());
	assert_int_equal(graphics.ref, 0);
	assert_int_equal(gs_stagesurface_get_width(surface), 0);
	assert_diagnostic(names[0], "called while not in a graphics context", 10);
	assert_int_equal(gs_stagesurface_get_height(surface), 0);
	assert_diagnostic(names[1], "called while not in a graphics context", 11);
	assert_int_equal(gs_stagesurface_get_color_format(surface), GS_UNKNOWN);
	assert_diagnostic(names[2], "called while not in a graphics context", 12);
	for (size_t i = 0; i < 3; i++)
		assert_int_equal(calls[i], 4);
	base_set_log_handler(NULL, NULL);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(getters)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
