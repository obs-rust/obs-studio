/* Characterize the public C cube texture access and destroy wrappers with a fake backend.
 * No GPU allocations: the opaque texture is only compared by address. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

static struct graphics_subsystem graphics;
static unsigned char token;
static unsigned calls[3], logs;
static int last_level;
static char last_log[160];
static uint32_t size;
static enum gs_color_format color;

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	logs++;
	last_level = level;
	vsnprintf(last_log, sizeof(last_log), format, args);
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, graphics.device);
}

static uint32_t backend_size(const gs_texture_t *tex)
{
	assert_ptr_equal(tex, &token);
	calls[0]++;
	return size;
}

static void backend_destroy(gs_texture_t *tex)
{
	assert_ptr_equal(tex, &token);
	calls[1]++;
	/* Backend owns release; the wrapper must not free or modify this token. */
	assert_int_equal(token, 0x5a);
	token = 0xa5;
}

static enum gs_color_format backend_color(const gs_texture_t *tex)
{
	assert_ptr_equal(tex, &token);
	calls[2]++;
	return color;
}

static int setup(void **state)
{
	(void)state;
	memset(&graphics, 0, sizeof(graphics));
	memset(calls, 0, sizeof(calls));
	logs = 0;
	token = 0x5a;
	graphics.device = (gs_device_t *)&graphics;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.gs_cubetexture_get_size = backend_size;
	graphics.exports.gs_cubetexture_destroy = backend_destroy;
	graphics.exports.gs_cubetexture_get_color_format = backend_color;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_set_log_handler(capture_log, NULL);
	assert_null(gs_get_context());
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	base_set_log_handler(NULL, NULL);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void diagnostic(unsigned index, const char *reason)
{
	static const char *names[] = {"gs_cubetexture_get_size", "gs_cubetexture_destroy",
				      "gs_cubetexture_get_color_format"};
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: %s", names[index], reason);
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_DEBUG);
	assert_string_equal(last_log, expected);
	assert_int_equal(calls[0] + calls[1] + calls[2], 0);
	logs = 0;
}

static void rejected_inputs(void **state)
{
	(void)state;
	/* Characterized, not endorsed: missing context masks a NULL pointer. */
	for (unsigned active = 0; active < 2; active++) {
		if (active)
			gs_enter_context(&graphics);
		for (unsigned nonnull = 0; nonnull < 2; nonnull++) {
			if (active && nonnull)
				continue;
			gs_texture_t *tex = nonnull ? (gs_texture_t *)&token : NULL;
			const char *reason = active ? "Null 'cubetex' parameter"
						    : "called while not in a graphics context";
			assert_int_equal(gs_cubetexture_get_size(tex), 0);
			diagnostic(0, reason);
			gs_cubetexture_destroy(tex);
			if (active) {
				assert_int_equal(logs, 0);
				assert_int_equal(calls[1], 0);
			} else {
				diagnostic(1, reason);
			}
			assert_int_equal(gs_cubetexture_get_color_format(tex), GS_UNKNOWN);
			diagnostic(2, reason);
		}
	}
}

static void backend_results(void **state)
{
	(void)state;
	const uint32_t values[] = {0, 1, 1920, UINT32_MAX};
	const enum gs_color_format colors[] = {GS_UNKNOWN, GS_RGBA, GS_R8, GS_RGBA16F};
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		size = values[i];
		color = colors[i];
		assert_int_equal(gs_cubetexture_get_size((const gs_texture_t *)&token), size);
		assert_int_equal(calls[0], i + 1);
		assert_int_equal(calls[1], 0);
		assert_int_equal(calls[2], i);
		assert_int_equal(token, 0x5a);
		assert_int_equal(gs_cubetexture_get_color_format((const gs_texture_t *)&token), color);
		assert_int_equal(calls[2], i + 1);
		assert_int_equal(logs, 0);
	}
	gs_cubetexture_destroy((gs_texture_t *)&token);
	assert_int_equal(calls[1], 1);
	assert_int_equal(calls[0], 4);
	assert_int_equal(calls[2], 4);
	assert_int_equal(token, 0xa5);
	assert_int_equal(logs, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(rejected_inputs, setup, teardown),
					   cmocka_unit_test_setup_teardown(backend_results, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
