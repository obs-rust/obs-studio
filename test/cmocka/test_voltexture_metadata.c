/* Characterize the public C volume texture metadata wrappers with a fake backend.
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
static unsigned calls[4], logs;
static int last_level;
static char last_log[160];
static uint32_t width, height, depth;
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

static uint32_t backend_width(const gs_texture_t *tex)
{
	assert_ptr_equal(tex, &token);
	calls[0]++;
	return width;
}

static uint32_t backend_height(const gs_texture_t *tex)
{
	assert_ptr_equal(tex, &token);
	calls[1]++;
	return height;
}

static uint32_t backend_depth(const gs_texture_t *tex)
{
	assert_ptr_equal(tex, &token);
	calls[2]++;
	return depth;
}

static enum gs_color_format backend_color(const gs_texture_t *tex)
{
	assert_ptr_equal(tex, &token);
	calls[3]++;
	return color;
}

static int setup(void **state)
{
	(void)state;
	memset(&graphics, 0, sizeof(graphics));
	memset(calls, 0, sizeof(calls));
	logs = 0;
	graphics.device = (gs_device_t *)&graphics;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.gs_voltexture_get_width = backend_width;
	graphics.exports.gs_voltexture_get_height = backend_height;
	graphics.exports.gs_voltexture_get_depth = backend_depth;
	graphics.exports.gs_voltexture_get_color_format = backend_color;
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
	static const char *names[] = {"gs_voltexture_get_width", "gs_voltexture_get_height", "gs_voltexture_get_depth",
				      "gs_voltexture_get_color_format"};
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: %s", names[index], reason);
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_DEBUG);
	assert_string_equal(last_log, expected);
	assert_int_equal(calls[0] + calls[1] + calls[2] + calls[3], 0);
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
			const gs_texture_t *tex = nonnull ? (const gs_texture_t *)&token : NULL;
			const char *reason = active ? "Null 'voltex' parameter"
						    : "called while not in a graphics context";
			assert_int_equal(gs_voltexture_get_width(tex), 0);
			diagnostic(0, reason);
			assert_int_equal(gs_voltexture_get_height(tex), 0);
			diagnostic(1, reason);
			assert_int_equal(gs_voltexture_get_depth(tex), 0);
			diagnostic(2, reason);
			assert_int_equal(gs_voltexture_get_color_format(tex), GS_UNKNOWN);
			diagnostic(3, reason);
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
		width = values[i];
		height = values[3 - i];
		depth = values[(i + 1) % 4];
		color = colors[i];
		assert_int_equal(gs_voltexture_get_width((const gs_texture_t *)&token), width);
		assert_int_equal(calls[0], i + 1);
		assert_int_equal(calls[1], i);
		assert_int_equal(calls[2], i);
		assert_int_equal(calls[3], i);
		assert_int_equal(gs_voltexture_get_height((const gs_texture_t *)&token), height);
		assert_int_equal(calls[1], i + 1);
		assert_int_equal(gs_voltexture_get_depth((const gs_texture_t *)&token), depth);
		assert_int_equal(calls[2], i + 1);
		assert_int_equal(gs_voltexture_get_color_format((const gs_texture_t *)&token), color);
		assert_int_equal(calls[3], i + 1);
		assert_int_equal(logs, 0);
	}
}

static void enum_abi(void **state)
{
	(void)state;
	/* Public enum ABI: pin every enumerator and its C integer representation. */
	assert_int_equal(sizeof(enum gs_color_format), sizeof(int));
	assert_int_equal(_Alignof(enum gs_color_format), _Alignof(int));
	const enum gs_color_format formats[] = {
		GS_UNKNOWN,     GS_A8,         GS_R8,         GS_RGBA,       GS_BGRX,    GS_BGRA,
		GS_R10G10B10A2, GS_RGBA16,     GS_R16,        GS_RGBA16F,    GS_RGBA32F, GS_RG16F,
		GS_RG32F,       GS_R16F,       GS_R32F,       GS_DXT1,       GS_DXT3,    GS_DXT5,
		GS_R8G8,        GS_RGBA_UNORM, GS_BGRX_UNORM, GS_BGRA_UNORM, GS_RG16,
	};
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < sizeof(formats) / sizeof(formats[0]); i++) {
		assert_int_equal(formats[i], i);
		color = formats[i];
		assert_int_equal(gs_voltexture_get_color_format((const gs_texture_t *)&token), i);
		assert_int_equal(calls[3], i + 1);
		assert_int_equal(calls[0] + calls[1] + calls[2], 0);
		assert_int_equal(logs, 0);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(rejected_inputs, setup, teardown),
					   cmocka_unit_test_setup_teardown(backend_results, setup, teardown),
					   cmocka_unit_test_setup_teardown(enum_abi, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
