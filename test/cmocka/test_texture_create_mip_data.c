#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>

#include <graphics/graphics-internal.h>
#include <util/base.h>
#include <util/bmem.h>

/*
 * Characterization of the mipmap rules of gs_texture_create() and
 * gs_cubetexture_create() (libobs/graphics/graphics.c.h, the 2D one through
 * gs_texture_mip_plan()), before the cube rule is ported (#2859).
 *
 * No GPU. The fixture builds a graphics_t by hand, as test_texture_render
 * does, and its fake device records the size, levels, flags and data each
 * creator passes on. The log handler records each warning. The creators
 * are called through libobs's exported symbols, so the same test runs
 * against the C build, the C reference and hybrid libobs.
 *
 * Both creators turn mipmaps off for a size that is not a power of two
 * (1 included), else for a render target: GS_BUILD_MIPMAPS is cleared and
 * the texture gets 1 level. They differ on the initial data of a render
 * target, as upstream always has: the 2D creator passes it on, the cube
 * creator drops it. The backends read the data face by face, each face
 * with all the levels the caller asked for, so after the cut to 1 level
 * entry 0 is still a 2D texture's top level, while a cube's face 1 would
 * read the caller's second level of face 0.
 */

static char device_token;
static char texture_token;
static const uint8_t pixels[4][4];
static const uint8_t *data[4] = {pixels[0], pixels[1], pixels[2], pixels[3]};

#define LOG_SIZE 1024

static char call_log[LOG_SIZE];
static char warnings[LOG_SIZE];

/* printf-checked, so clang's -Wformat-nonliteral accepts the vsnprintf. */
#if defined(__GNUC__) || defined(__clang__)
#define RECORD_FORMAT __attribute__((format(printf, 2, 3)))
#else
#define RECORD_FORMAT
#endif

RECORD_FORMAT static void append(char *buf, const char *format, ...)
{
	size_t len = strlen(buf);
	va_list args;

	if (len)
		buf[len++] = ';';

	va_start(args, format);
	vsnprintf(buf + len, LOG_SIZE - len, format, args);
	va_end(args);
}

static const char *data_name(const uint8_t **p)
{
	if (!p)
		return "null";
	if (p == data)
		return "data";
	return "?";
}

static void capture_log(int level, const char *format, va_list args, void *param)
{
	char line[256];

	(void)param;
	vsnprintf(line, sizeof(line), format, args);
	if (level == LOG_WARNING)
		append(warnings, "%s", strstr(line, "render targets") ? "render target" : "not pow2");
}

static gs_texture_t *fake_texture_create(gs_device_t *device, uint32_t width, uint32_t height,
					 enum gs_color_format color_format, uint32_t levels, const uint8_t **p,
					 uint32_t flags)
{
	assert_ptr_equal(device, &device_token);
	append(call_log, "tex %ux%u f%d l%u %s 0x%x", width, height, (int)color_format, levels, data_name(p), flags);
	return (gs_texture_t *)&texture_token;
}

static gs_texture_t *fake_cubetexture_create(gs_device_t *device, uint32_t size, enum gs_color_format color_format,
					     uint32_t levels, const uint8_t **p, uint32_t flags)
{
	assert_ptr_equal(device, &device_token);
	append(call_log, "cube %u f%d l%u %s 0x%x", size, (int)color_format, levels, data_name(p), flags);
	return (gs_texture_t *)&texture_token;
}

static void fake_enter_context(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static void fake_leave_context(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static graphics_t *graphics;

static int setup(void **state)
{
	(void)state;

	graphics = bzalloc(sizeof(struct graphics_subsystem));
	pthread_mutex_init(&graphics->mutex, NULL);
	pthread_mutex_init(&graphics->effect_mutex, NULL);

	graphics->device = (gs_device_t *)&device_token;
	graphics->exports.device_enter_context = fake_enter_context;
	graphics->exports.device_leave_context = fake_leave_context;
	graphics->exports.device_texture_create = fake_texture_create;
	graphics->exports.device_cubetexture_create = fake_cubetexture_create;

	call_log[0] = 0;
	warnings[0] = 0;
	base_set_log_handler(capture_log, NULL);
	gs_enter_context(graphics);
	return 0;
}

static int teardown(void **state)
{
	(void)state;

	gs_leave_context();
	base_set_log_handler(NULL, NULL);
	pthread_mutex_destroy(&graphics->mutex);
	pthread_mutex_destroy(&graphics->effect_mutex);
	bfree(graphics);
	graphics = NULL;
	return 0;
}

static void clear_logs(void)
{
	call_log[0] = 0;
	warnings[0] = 0;
}

/* A render target asking for mipmaps keeps its data in 2D, by flag or by
 * levels (0 is the full chain). */
static void texture_render_target_test(void **state)
{
	(void)state;

	assert_ptr_equal(gs_texture_create(64, 32, GS_RGBA, 1, data, GS_RENDER_TARGET | GS_BUILD_MIPMAPS),
			 &texture_token);
	assert_string_equal(call_log, "tex 64x32 f3 l1 data 0x4");
	assert_string_equal(warnings, "render target");

	clear_logs();
	gs_texture_create(64, 32, GS_RGBA, 0, data, GS_RENDER_TARGET | GS_DYNAMIC);
	assert_string_equal(call_log, "tex 64x32 f3 l1 data 0x6");
	assert_string_equal(warnings, "render target");

	clear_logs();
	gs_texture_create(64, 32, GS_RGBA, 7, NULL, GS_RENDER_TARGET);
	assert_string_equal(call_log, "tex 64x32 f3 l1 null 0x4");
	assert_string_equal(warnings, "render target");
}

/* The same render targets as a cube lose their data. */
static void cube_render_target_test(void **state)
{
	(void)state;

	assert_ptr_equal(gs_cubetexture_create(64, GS_RGBA, 1, data, GS_RENDER_TARGET | GS_BUILD_MIPMAPS),
			 &texture_token);
	assert_string_equal(call_log, "cube 64 f3 l1 null 0x4");
	assert_string_equal(warnings, "render target");

	clear_logs();
	gs_cubetexture_create(64, GS_RGBA, 0, data, GS_RENDER_TARGET | GS_DYNAMIC);
	assert_string_equal(call_log, "cube 64 f3 l1 null 0x6");
	assert_string_equal(warnings, "render target");

	clear_logs();
	gs_cubetexture_create(64, GS_RGBA, 7, NULL, GS_RENDER_TARGET);
	assert_string_equal(call_log, "cube 64 f3 l1 null 0x4");
	assert_string_equal(warnings, "render target");
}

/* A size that is not a power of two warns about that instead, once, and
 * the render target check never runs. */
static void not_pow2_test(void **state)
{
	(void)state;

	gs_texture_create(100, 64, GS_RGBA, 1, data, GS_RENDER_TARGET | GS_BUILD_MIPMAPS);
	assert_string_equal(call_log, "tex 100x64 f3 l1 data 0x4");
	assert_string_equal(warnings, "not pow2");

	clear_logs();
	gs_texture_create(64, 100, GS_RGBA, 7, data, 0);
	assert_string_equal(call_log, "tex 64x100 f3 l1 data 0x0");
	assert_string_equal(warnings, "not pow2");

	clear_logs();
	gs_texture_create(1, 1, GS_RGBA, 1, data, GS_BUILD_MIPMAPS);
	assert_string_equal(call_log, "tex 1x1 f3 l1 data 0x0");
	assert_string_equal(warnings, "not pow2");

	clear_logs();
	gs_cubetexture_create(100, GS_RGBA, 7, NULL, GS_RENDER_TARGET | GS_BUILD_MIPMAPS);
	assert_string_equal(call_log, "cube 100 f3 l1 null 0x4");
	assert_string_equal(warnings, "not pow2");

	clear_logs();
	gs_cubetexture_create(1, GS_RGBA, 1, NULL, GS_BUILD_MIPMAPS);
	assert_string_equal(call_log, "cube 1 f3 l1 null 0x0");
	assert_string_equal(warnings, "not pow2");
}

/* Without a mipmap request, or without a render target, nothing changes:
 * the levels, flags and data pass through and nothing warns. */
static void unchanged_test(void **state)
{
	(void)state;

	gs_texture_create(64, 32, GS_RGBA, 1, data, GS_RENDER_TARGET);
	gs_texture_create(100, 64, GS_RGBA, 1, data, GS_DYNAMIC);
	gs_texture_create(64, 32, GS_RGBA, 7, data, GS_BUILD_MIPMAPS);
	gs_texture_create(64, 32, GS_RGBA, 0, data, 0);
	gs_cubetexture_create(64, GS_RGBA, 1, data, GS_RENDER_TARGET);
	gs_cubetexture_create(100, GS_RGBA, 1, data, GS_DYNAMIC);
	gs_cubetexture_create(64, GS_RGBA, 7, data, GS_BUILD_MIPMAPS);
	gs_cubetexture_create(64, GS_RGBA, 0, data, 0);
	assert_string_equal(call_log, "tex 64x32 f3 l1 data 0x4;"
				      "tex 100x64 f3 l1 data 0x2;"
				      "tex 64x32 f3 l7 data 0x1;"
				      "tex 64x32 f3 l0 data 0x0;"
				      "cube 64 f3 l1 data 0x4;"
				      "cube 100 f3 l1 data 0x2;"
				      "cube 64 f3 l7 data 0x1;"
				      "cube 64 f3 l0 data 0x0");
	assert_string_equal(warnings, "");
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(texture_render_target_test, setup, teardown),
		cmocka_unit_test_setup_teardown(cube_render_target_test, setup, teardown),
		cmocka_unit_test_setup_teardown(not_pow2_test, setup, teardown),
		cmocka_unit_test_setup_teardown(unchanged_test, setup, teardown),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
