#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <string.h>
#include <dlfcn.h>
#include <cmocka.h>

#include <graphics/graphics-internal.h>
#include <util/platform.h>
#include <util/bmem.h>

/* Public APIs of the loaded libobs, with no display or GPU. ELF interposition
 * replaces only backend callbacks used by gs_create/init/destroy and these
 * tests. Unused imports resolve to the real OpenGL module, never placeholders.
 * In particular, production load_graphics_imports and gs_create still run.
 * This fixture is Linux-only because it relies on ELF symbol preemption. */
static char scratch[] = "graphics-viewport-XXXXXX";
static char image_path[128];
static void *backend;
static int token;
static int backend_type;
static bool has_max;
static uint32_t max_size;
static unsigned type_calls, max_calls, buffers;
static uint32_t target_width, target_height;
static char calls[16];
static size_t call_count;
static int viewport[4];
static float projection[6];

static void record(char call)
{
	assert_true(call_count + 1 < sizeof(calls));
	calls[call_count++] = call;
	calls[call_count] = 0;
}

static void check_device(const gs_device_t *device)
{
	assert_ptr_equal(device, &token);
}

static int fake_create(gs_device_t **device, uint32_t adapter)
{
	assert_int_equal(adapter, 0);
	*device = (gs_device_t *)&token;
	return GS_SUCCESS;
}

static void fake_context(gs_device_t *device)
{
	check_device(device);
}

static void fake_destroy(gs_device_t *device)
{
	check_device(device);
	assert_int_equal(buffers, 0);
}

static int fake_type(void)
{
	type_calls++;
	return backend_type;
}

static uint32_t fake_max(gs_device_t *device)
{
	check_device(device);
	max_calls++;
	return max_size;
}

static gs_vertbuffer_t *fake_buffer(gs_device_t *device, struct gs_vb_data *data, uint32_t flags)
{
	check_device(device);
	(void)flags;
	buffers++;
	return (gs_vertbuffer_t *)data;
}

static void fake_free_buffer(gs_vertbuffer_t *buffer)
{
	assert_non_null(buffer);
	assert_true(buffers > 0);
	buffers--;
	gs_vbdata_destroy((struct gs_vb_data *)buffer);
}

static void fake_blend(gs_device_t *device, enum gs_blend_type src_c, enum gs_blend_type dst_c,
		       enum gs_blend_type src_a, enum gs_blend_type dst_a)
{
	check_device(device);
	assert_int_equal(src_c, GS_BLEND_SRCALPHA);
	assert_int_equal(dst_c, GS_BLEND_INVSRCALPHA);
	assert_int_equal(src_a, GS_BLEND_ONE);
	assert_int_equal(dst_a, GS_BLEND_INVSRCALPHA);
}

static void fake_blend_op(gs_device_t *device, enum gs_blend_op_type op)
{
	check_device(device);
	assert_int_equal(op, GS_BLEND_OP_ADD);
}

static void fake_size(const gs_device_t *device, uint32_t *width, uint32_t *height)
{
	check_device(device);
	record('s');
	*width = target_width;
	*height = target_height;
}

static void fake_viewport(gs_device_t *device, int x, int y, int width, int height)
{
	check_device(device);
	record('v');
	viewport[0] = x;
	viewport[1] = y;
	viewport[2] = width;
	viewport[3] = height;
}

static void fake_ortho(gs_device_t *device, float left, float right, float top, float bottom, float znear, float zfar)
{
	check_device(device);
	record('o');
	projection[0] = left;
	projection[1] = right;
	projection[2] = top;
	projection[3] = bottom;
	projection[4] = znear;
	projection[5] = zfar;
}

/* POSIX dlsym conversions use unions as the production graphics loader does. */
void *os_dlopen(const char *path)
{
	union {
		void *obj;
		void *(*fn)(const char *);
	} next;
	next.obj = dlsym(RTLD_NEXT, "os_dlopen");
	assert_non_null(next.obj);
	void *module = next.fn(path);
	if (strcmp(path, VIEWPORT_BACKEND_PATH) == 0) {
		assert_non_null(module);
		assert_null(backend);
		backend = module;
	}
	return module;
}

void *os_dlsym(void *module, const char *name)
{
	if (module == backend && backend) {
#define REPLACE(symbol, callback)                          \
	if (strcmp(name, #symbol) == 0) {                  \
		union {                                   \
			void *obj;                        \
			__typeof__(&callback) fn;          \
		} replacement;                            \
		replacement.fn = &callback;                \
		return replacement.obj;                   \
	}
		REPLACE(device_create, fake_create)
		REPLACE(device_destroy, fake_destroy)
		REPLACE(device_enter_context, fake_context)
		REPLACE(device_leave_context, fake_context)
		REPLACE(device_get_type, fake_type)
		REPLACE(device_vertexbuffer_create, fake_buffer)
		REPLACE(gs_vertexbuffer_destroy, fake_free_buffer)
		REPLACE(device_blend_function_separate, fake_blend)
		REPLACE(device_blend_op, fake_blend_op)
		REPLACE(device_get_size, fake_size)
		REPLACE(device_set_viewport, fake_viewport)
		REPLACE(device_ortho, fake_ortho)
		if (!has_max && strcmp(name, "device_get_max_texture_size") == 0)
			return NULL;
		REPLACE(device_get_max_texture_size, fake_max)
#undef REPLACE
	}
	union {
		void *obj;
		void *(*fn)(void *, const char *);
	} next;
	next.obj = dlsym(RTLD_NEXT, "os_dlsym");
	assert_non_null(next.obj);
	return next.fn(module, name);
}

/* The cached maximum is private. Observe it through the public CPU image
 * loader: a one-row PPM at the limit loads, one pixel beyond is rejected.
 * Zero means no known limit, so a row beyond the D3D11/Metal limit loads. */
static void image_width(uint32_t width, bool accepted)
{
	FILE *file = fopen(image_path, "wb");
	assert_non_null(file);
	assert_true(fprintf(file, "P6\n%u 1\n255\n", width) > 0);
	unsigned char *row = bzalloc((size_t)width * 3);
	assert_int_equal(fwrite(row, 3, width, file), width);
	bfree(row);
	assert_int_equal(fclose(file), 0);
	enum gs_color_format format = GS_UNKNOWN;
	uint32_t cx = 0, cy = 0;
	uint8_t *pixels = gs_create_texture_file_data(image_path, &format, &cx, &cy);
	if (accepted) {
		assert_non_null(pixels);
		assert_int_equal(cx, width);
		assert_int_equal(cy, 1);
	} else {
		assert_null(pixels);
		assert_int_equal(cx, 0);
		assert_int_equal(cy, 0);
	}
	bfree(pixels);
}

static void assert_limit(uint32_t expected)
{
	if (expected == 0) {
		image_width(16385, true);
	} else {
		image_width(expected, true);
		image_width(expected + 1, false);
	}
}

static graphics_t *create(int type, bool optional, uint32_t maximum, uint32_t expected)
{
	graphics_t *graphics = NULL;
	assert_null(gs_get_context());
	backend = NULL;
	backend_type = type;
	has_max = optional;
	max_size = maximum;
	type_calls = max_calls = 0;
	assert_int_equal(gs_create(&graphics, VIEWPORT_BACKEND_PATH, 0), GS_SUCCESS);
	assert_non_null(graphics);
	assert_limit(expected);
	assert_int_equal(max_calls, optional ? 1 : 0);
	assert_int_equal(type_calls, optional ? 0 : 1);
	assert_null(gs_get_context());
	return graphics;
}

static void destroy(graphics_t *graphics)
{
	gs_destroy(graphics);
	backend = NULL;
	assert_int_equal(buffers, 0);
	assert_null(gs_get_context());
	assert_limit(0);
}

static void capability_fallback(void **state)
{
	(void)state;
	const int types[] = {GS_DEVICE_OPENGL, GS_DEVICE_DIRECT3D_11, GS_DEVICE_METAL, -1, 99};
	for (size_t i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
		uint32_t fallback = types[i] == GS_DEVICE_DIRECT3D_11 || types[i] == GS_DEVICE_METAL ? 16384 : 0;
		destroy(create(types[i], false, 123, fallback));
		/* An optional callback returning zero still wins over the fallback. */
		destroy(create(types[i], true, 0, 0));
		destroy(create(types[i], true, 8192, 8192));
		destroy(create(types[i], true, 7, 7));
	}
}

static void viewport_and_projection(void **state)
{
	(void)state;
	graphics_t *graphics = create(GS_DEVICE_OPENGL, true, 4096, 4096);
	const uint32_t sizes[][2] = {{1920, 1080}, {0, 0}, {1, 7}, {65535, 32769}, {16777217, 16777219}};
	gs_enter_context(graphics);
	for (size_t i = 0; i < sizeof(sizes) / sizeof(sizes[0]); i++) {
		target_width = sizes[i][0];
		target_height = sizes[i][1];
		call_count = 0;
		calls[0] = 0;
		gs_reset_viewport();
		assert_string_equal(calls, "sv");
		assert_int_equal(viewport[0], 0);
		assert_int_equal(viewport[1], 0);
		assert_int_equal(viewport[2], (int)target_width);
		assert_int_equal(viewport[3], (int)target_height);
		/* Every call must query the current size; no cached dimensions. */
		gs_set_2d_mode();
		assert_string_equal(calls, "svso");
		assert_true(projection[0] == 0.0f);
		assert_true(projection[1] == (float)target_width);
		assert_true(projection[2] == 0.0f);
		assert_true(projection[3] == (float)target_height);
		/* Characterized, not endorsed: both depths are negative. */
		assert_true(projection[4] == -1.0f);
		assert_true(projection[5] == -1024.0f);
	}
	gs_leave_context();
	call_count = 0;
	calls[0] = 0;
	gs_reset_viewport();
	gs_set_2d_mode();
	assert_string_equal(calls, "");
	destroy(graphics);
}

int main(void)
{
	assert_non_null(mkdtemp(scratch));
	assert_true(snprintf(image_path, sizeof(image_path), "%s/row.ppm", scratch) > 0);
	const struct CMUnitTest tests[] = {cmocka_unit_test(capability_fallback),
					   cmocka_unit_test(viewport_and_projection)};
	int result = cmocka_run_group_tests(tests, NULL, NULL);
	assert_int_equal(remove(image_path), 0);
	assert_int_equal(rmdir(scratch), 0);
	return result;
}
