/* Characterize the real libobs planar texture entry points, without a GPU.
 * Backend callbacks own only stable token addresses, never GPU allocations. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>

#include "graphics/graphics-internal.h"
#include "util/base.h"

static struct graphics_subsystem graphics;
static unsigned char tokens[4];
static unsigned native_calls, create_calls, destroy_calls, failures;
static unsigned logs;
static int last_level;
static char last_log[128];
static bool native_success, p010;
static uint32_t expected_width, expected_height, expected_flags;
static gs_texture_t *destroyed[2];

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

static gs_texture_t *token(unsigned index)
{
	return (gs_texture_t *)&tokens[index];
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, graphics.device);
}

static bool native_create(gs_device_t *device, gs_texture_t **y, gs_texture_t **uv, uint32_t width, uint32_t height,
			  uint32_t flags)
{
	assert_ptr_equal(device, graphics.device);
	assert_int_equal(width, expected_width);
	assert_int_equal(height, expected_height);
	assert_int_equal(flags, expected_flags);
	native_calls++;
	/* Even on decline the fallback must replace backend-written outputs. */
	*y = token(2);
	*uv = token(3);
	return native_success;
}

static gs_texture_t *create_plane(gs_device_t *device, uint32_t width, uint32_t height, enum gs_color_format format,
				  uint32_t levels, const uint8_t **data, uint32_t flags)
{
	unsigned plane = create_calls++;
	assert_true(plane < 2);
	assert_ptr_equal(device, graphics.device);
	assert_int_equal(width, plane ? expected_width / 2 : expected_width);
	assert_int_equal(height, plane ? expected_height / 2 : expected_height);
	assert_int_equal(format, p010 ? (plane ? GS_RG16 : GS_R16) : (plane ? GS_R8G8 : GS_R8));
	assert_int_equal(levels, 1);
	assert_null(data);
	assert_int_equal(flags, expected_flags);
	return (failures & (1u << plane)) ? NULL : token(plane);
}

static void destroy_plane(gs_texture_t *texture)
{
	assert_true(destroy_calls < 2);
	destroyed[destroy_calls++] = texture;
}

static bool create_pair(gs_texture_t **y, gs_texture_t **uv, uint32_t width, uint32_t height, uint32_t flags)
{
	return p010 ? gs_texture_create_p010(y, uv, width, height, flags)
		    : gs_texture_create_nv12(y, uv, width, height, flags);
}

static void reset(void)
{
	native_calls = create_calls = destroy_calls = logs = 0;
	memset(destroyed, 0, sizeof(destroyed));
}

static int setup(void **state)
{
	(void)state;
	memset(&graphics, 0, sizeof(graphics));
	graphics.device = (gs_device_t *)&graphics;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.device_texture_create = create_plane;
	graphics.exports.gs_texture_destroy = destroy_plane;
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

static void rejected_inputs(void **state)
{
	(void)state;
	for (unsigned format = 0; format < 2; format++) {
		p010 = format != 0;
		gs_texture_t *y = token(2), *uv = token(3);
		reset();
		assert_false(create_pair(&y, &uv, 3, 5, GS_DYNAMIC));
		assert_ptr_equal(y, token(2));
		assert_ptr_equal(uv, token(3));
		assert_int_equal(logs, 1);
		assert_int_equal(last_level, LOG_DEBUG);
		assert_string_equal(last_log, p010 ? "gs_texture_create_p010: called while not in a graphics context"
						   : "gs_texture_create_nv12: called while not in a graphics context");
		gs_enter_context(&graphics);
		graphics.exports.device_texture_create_nv12 = native_create;
		graphics.exports.device_texture_create_p010 = native_create;
		const uint32_t dimensions[][2] = {{3, 4}, {4, 3}, {3, 3}, {UINT32_MAX, 2}, {2, UINT32_MAX}};
		for (unsigned i = 0; i < sizeof(dimensions) / sizeof(dimensions[0]); i++) {
			reset();
			assert_false(create_pair(&y, &uv, dimensions[i][0], dimensions[i][1], GS_DYNAMIC));
			assert_ptr_equal(y, token(2));
			assert_ptr_equal(uv, token(3));
			assert_int_equal(logs, 1);
			assert_int_equal(last_level, LOG_ERROR);
			assert_string_equal(last_log, p010 ? "P010 textures must have dimensions divisible by 2."
							   : "NV12 textures must have dimensions divisible by 2.");
			assert_int_equal(native_calls, 0);
			assert_int_equal(create_calls, 0);
			assert_int_equal(destroy_calls, 0);
		}
		gs_leave_context();
	}
}

static void native_and_fallback(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	const uint32_t dimensions[][2] = {{1920, 1080}, {2, 2}, {0, 0}, {0, 2}, {2, 0}};
	const uint32_t flags[] = {0, GS_DYNAMIC, GS_RENDER_TARGET};
	for (unsigned format = 0; format < 2; format++) {
		p010 = format != 0;
		for (unsigned native = 0; native < 3; native++) {
			graphics.exports.device_texture_create_nv12 = native ? native_create : NULL;
			graphics.exports.device_texture_create_p010 = native ? native_create : NULL;
			native_success = native == 2;
			for (unsigned d = 0; d < sizeof(dimensions) / sizeof(dimensions[0]); d++) {
				expected_width = dimensions[d][0];
				expected_height = dimensions[d][1];
				for (unsigned f = 0; f < sizeof(flags) / sizeof(flags[0]); f++) {
					expected_flags = flags[f];
					for (failures = 0; failures < 4; failures++) {
						reset();
						gs_texture_t *y = token(2), *uv = token(3);
						bool success = create_pair(&y, &uv, expected_width, expected_height,
									   expected_flags);
						assert_int_equal(success, native_success || failures == 0);
						assert_int_equal(native_calls, native != 0);
						assert_int_equal(create_calls, native_success ? 0 : 2);
						assert_int_equal(logs, 0);
						if (success) {
							assert_ptr_equal(y, token(native_success ? 2 : 0));
							assert_ptr_equal(uv, token(native_success ? 3 : 1));
							assert_int_equal(destroy_calls, 0);
						} else {
							assert_null(y);
							assert_null(uv);
							assert_int_equal(destroy_calls, failures == 3 ? 0 : 1);
							if (destroy_calls)
								assert_ptr_equal(destroyed[0],
										 token(failures == 1 ? 1 : 0));
						}
					}
				}
			}
		}
	}
	/* Zero dimensions reaching the backend is characterized, not endorsed. */
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(rejected_inputs, setup, teardown),
					   cmocka_unit_test_setup_teardown(native_and_fallback, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
