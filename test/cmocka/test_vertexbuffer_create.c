/* Pin C allocation/copy policy before the obs-graphics Rust swap (#4766). */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "graphics/graphics-internal.h"
#include "util/base.h"

static struct gs_vb_data *seen_data;
static gs_device_t *seen_device;
static uint32_t seen_flags;
static unsigned calls, logs;
static char log_text[128];
static int log_level;
static int device_token, result_token;
static bool backend_failure;

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
	assert_ptr_equal(device, &device_token);
}

static gs_vertbuffer_t *backend_create(gs_device_t *device, struct gs_vb_data *data, uint32_t flags)
{
	calls++;
	seen_device = device;
	seen_data = data;
	seen_flags = flags;
	return backend_failure ? NULL : (gs_vertbuffer_t *)&result_token;
}

static void dispatch(struct gs_vb_data *data, uint32_t flags)
{
	unsigned previous = calls;
	gs_vertbuffer_t *result = gs_vertexbuffer_create(data, flags);
	assert_ptr_equal(result, backend_failure ? NULL : (gs_vertbuffer_t *)&result_token);
	assert_int_equal(calls, previous + 1);
	assert_ptr_equal(seen_device, &device_token);
	assert_int_equal(seen_flags, flags);
	assert_int_equal(logs, 2);
}

static void assert_copy(const void *copy, const void *original, size_t size)
{
	if (original) {
		assert_non_null(copy);
		assert_ptr_not_equal(copy, original);
		assert_memory_equal(copy, original, size);
	} else {
		assert_null(copy);
	}
}

static void ownership(void **state)
{
	(void)state;
	struct graphics_subsystem graphics = {0};
	struct vec3 points[3], normals[3], tangents[3];
	uint32_t colors[] = {0, UINT32_MAX, 0x12345678};
	float coords1[] = {1.0f, -2.0f, 3.0f};
	float coords2[] = {1, 2, 3, 4, 5, 6};
	float coords4[] = {1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12};
	struct gs_tvertarray tex[] = {{1, coords1}, {2, coords2}, {4, coords4}};
	struct gs_vb_data data = {0};
	/* Set every byte, including vec3 padding, to expose the exact copy size. */
	memset(points, 0x13, sizeof(points));
	memset(normals, 0x25, sizeof(normals));
	memset(tangents, 0x37, sizeof(tangents));
	data.num = 3;
	data.points = points;
	data.normals = normals;
	data.tangents = tangents;
	data.colors = colors;
	data.num_tex = 3;
	data.tvarray = tex;

	base_set_log_handler(capture_log, NULL);
	assert_null(gs_get_context());
	assert_null(gs_vertexbuffer_create(NULL, GS_DUP_BUFFER));
	assert_null(gs_vertexbuffer_create(&data, GS_DUP_BUFFER));
	assert_int_equal(calls, 0);
	assert_int_equal(logs, 2);
	assert_int_equal(log_level, LOG_DEBUG);
	assert_string_equal(log_text, "gs_vertexbuffer_create: called while not in a graphics context");

	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = device_context;
	graphics.exports.device_leave_context = device_context;
	graphics.exports.device_vertexbuffer_create = backend_create;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	gs_enter_context(&graphics);

	/* NULL and zero vertices bypass duplication, even with GS_DUP_BUFFER.
	 * Characterized, not endorsed: populated arrays with num == 0 pass through. */
	dispatch(NULL, GS_DUP_BUFFER | GS_DYNAMIC);
	assert_null(seen_data);
	data.num = 0;
	dispatch(&data, GS_DUP_BUFFER);
	assert_ptr_equal(seen_data, &data);
	data.num = 3;
	dispatch(&data, GS_DYNAMIC);
	assert_ptr_equal(seen_data, &data);

	/* Exercise every combination of the four independently optional arrays. */
	for (unsigned mask = 0; mask < 16; mask++) {
		data.points = (mask & 1) ? points : NULL;
		data.normals = (mask & 2) ? normals : NULL;
		data.tangents = (mask & 4) ? tangents : NULL;
		data.colors = (mask & 8) ? colors : NULL;
		dispatch(&data, GS_DUP_BUFFER | GS_DYNAMIC);
		struct gs_vb_data *copy = seen_data;
		assert_ptr_not_equal(copy, &data);
		assert_int_equal(copy->num, 3);
		assert_copy(copy->points, data.points, sizeof(points));
		assert_copy(copy->normals, data.normals, sizeof(normals));
		assert_copy(copy->tangents, data.tangents, sizeof(tangents));
		assert_copy(copy->colors, data.colors, sizeof(colors));
		assert_int_equal(copy->num_tex, 3);
		assert_ptr_not_equal(copy->tvarray, tex);
		for (size_t i = 0; i < 3; i++) {
			assert_int_equal(copy->tvarray[i].width, tex[i].width);
			assert_copy(copy->tvarray[i].array, tex[i].array, tex[i].width * sizeof(float) * data.num);
		}
		/* Both directions remain independent after backend ownership transfer. */
		((float *)copy->tvarray[0].array)[0] = 99.0f;
		assert_float_equal(coords1[0], 1.0f, 0.0f);
		coords2[5] = -42.0f;
		assert_float_equal(((float *)copy->tvarray[1].array)[5], 6.0f, 0.0f);
		coords2[5] = 6.0f;
		if (copy->colors) {
			copy->colors[2] = 99;
			assert_int_equal(colors[2], 0x12345678);
		}
		gs_vbdata_destroy(copy);
		assert_int_equal(data.num, 3);
		assert_ptr_equal(data.tvarray, tex);
	}

	/* Missing texture-array pointer OR zero array count leaves both copied
	 * metadata fields zero. Nonzero counts with NULL arrays are characterized. */
	for (unsigned kind = 0; kind < 3; kind++) {
		data.tvarray = kind == 0 ? NULL : tex;
		data.num_tex = kind == 0 ? 3 : 0;
		backend_failure = kind == 2;
		dispatch(&data, GS_DUP_BUFFER);
		assert_ptr_not_equal(seen_data, &data);
		assert_int_equal(seen_data->num, 3);
		assert_int_equal(seen_data->num_tex, 0);
		assert_null(seen_data->tvarray);
		/* The backend owns the duplicate even when it returns NULL. */
		gs_vbdata_destroy(seen_data);
	}

	gs_leave_context();
	assert_null(gs_get_context());
	assert_int_equal(graphics.ref, 0);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	base_set_log_handler(NULL, NULL);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(ownership)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
