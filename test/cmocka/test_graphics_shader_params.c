/* Public shader write characterization (#4420). Recording backend, no GPU. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <float.h>
#include <limits.h>
#include <math.h>
#include <stdio.h>
#include <string.h>

#include "graphics/graphics-internal.h"
#include "graphics/vec2.h"
#include "util/base.h"

static unsigned calls, logs, expected_kind;
static gs_sparam_t *seen_param;
static const void *seen_value;
static bool seen_bool;
static float seen_float;
static int seen_int, log_level;
static char log_text[160];

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *data)
{
	(void)data;
	logs++;
	log_level = level;
	vsnprintf(log_text, sizeof(log_text), format, args);
}

static void record(unsigned kind, gs_sparam_t *param)
{
	assert_int_equal(kind, expected_kind);
	calls++;
	seen_param = param;
}

static void record_bool(gs_sparam_t *param, bool val)
{
	record(0, param);
	seen_bool = val;
}

static void record_float(gs_sparam_t *param, float val)
{
	record(1, param);
	seen_float = val;
}

static void record_int(gs_sparam_t *param, int val)
{
	record(2, param);
	seen_int = val;
}

static void record_matrix3(gs_sparam_t *param, const struct matrix3 *val)
{
	record(3, param);
	seen_value = val;
}

static void record_matrix4(gs_sparam_t *param, const struct matrix4 *val)
{
	record(4, param);
	seen_value = val;
}

static void record_vec2(gs_sparam_t *param, const struct vec2 *val)
{
	record(5, param);
	seen_value = val;
}

static void record_vec3(gs_sparam_t *param, const struct vec3 *val)
{
	record(6, param);
	seen_value = val;
}

static void record_vec4(gs_sparam_t *param, const struct vec4 *val)
{
	record(7, param);
	seen_value = val;
}

static void device_context(gs_device_t *device)
{
	(void)device;
}

static void setup_graphics(struct graphics_subsystem *graphics)
{
	graphics->exports.device_enter_context = device_context;
	graphics->exports.device_leave_context = device_context;
	graphics->exports.gs_shader_set_bool = record_bool;
	graphics->exports.gs_shader_set_float = record_float;
	graphics->exports.gs_shader_set_int = record_int;
	graphics->exports.gs_shader_set_matrix3 = record_matrix3;
	graphics->exports.gs_shader_set_matrix4 = record_matrix4;
	graphics->exports.gs_shader_set_vec2 = record_vec2;
	graphics->exports.gs_shader_set_vec3 = record_vec3;
	graphics->exports.gs_shader_set_vec4 = record_vec4;
	assert_int_equal(pthread_mutex_init(&graphics->mutex, NULL), 0);
	base_set_log_handler(capture_log, NULL);
	calls = logs = 0;
	assert_null(gs_get_context());
}

static void teardown_graphics(struct graphics_subsystem *graphics)
{
	assert_null(gs_get_context());
	base_set_log_handler(NULL, NULL);
	assert_int_equal(pthread_mutex_destroy(&graphics->mutex), 0);
}

/* An opaque, never-dereferenced parameter token: the backend owns its layout. */
static int parameter_token;

static void invoke(unsigned kind, gs_sparam_t *param, bool value_present)
{
	const struct matrix3 m3 = {0};
	const struct matrix4 m4 = {0};
	const struct vec2 v2 = {0};
	const struct vec3 v3 = {0};
	const struct vec4 v4 = {0};
	switch (kind) {
	case 0:
		gs_shader_set_bool(param, true);
		break;
	case 1:
		gs_shader_set_float(param, -1.25f);
		break;
	case 2:
		gs_shader_set_int(param, INT_MIN);
		break;
	case 3:
		gs_shader_set_matrix3(param, value_present ? &m3 : NULL);
		break;
	case 4:
		gs_shader_set_matrix4(param, value_present ? &m4 : NULL);
		break;
	case 5:
		gs_shader_set_vec2(param, value_present ? &v2 : NULL);
		break;
	case 6:
		gs_shader_set_vec3(param, value_present ? &v3 : NULL);
		break;
	case 7:
		gs_shader_set_vec4(param, value_present ? &v4 : NULL);
		break;
	}
}

static const char *const names[] = {
	"gs_shader_set_bool",    "gs_shader_set_float", "gs_shader_set_int",  "gs_shader_set_matrix3",
	"gs_shader_set_matrix4", "gs_shader_set_vec2",  "gs_shader_set_vec3", "gs_shader_set_vec4",
};

static void rejected(unsigned kind, gs_sparam_t *param, bool value_present, const char *reason)
{
	char expected[160];
	unsigned before_calls = calls, before_logs = logs;
	invoke(kind, param, value_present);
	assert_int_equal(calls, before_calls);
	assert_int_equal(logs, before_logs + 1);
	assert_int_equal(log_level, LOG_DEBUG);
	snprintf(expected, sizeof(expected), "%s: %s", names[kind], reason);
	assert_string_equal(log_text, expected);
}

static void validation(void **state)
{
	(void)state;
	struct graphics_subsystem graphics = {0};
	gs_sparam_t *param = (gs_sparam_t *)&parameter_token;
	setup_graphics(&graphics);
	for (unsigned kind = 0; kind < 8; kind++) {
		/* Context wins over both null arguments, characterized, not endorsed. */
		for (unsigned has_param = 0; has_param < 2; has_param++) {
			for (unsigned has_value = 0; has_value < 2; has_value++)
				rejected(kind, has_param ? param : NULL, has_value != 0,
					 "called while not in a graphics context");
		}
	}
	gs_enter_context(&graphics);
	assert_ptr_equal(gs_get_context(), &graphics);
	for (unsigned kind = 0; kind < 8; kind++) {
		rejected(kind, NULL, true, "Null 'param' parameter");
		rejected(kind, NULL, false, "Null 'param' parameter");
		if (kind >= 3)
			rejected(kind, param, false, "Null 'val' parameter");
	}
	assert_int_equal(calls, 0);
	assert_int_equal(graphics.ref, 1);
	gs_leave_context();
	for (unsigned kind = 0; kind < 8; kind++)
		rejected(kind, param, true, "called while not in a graphics context");
	teardown_graphics(&graphics);
}

static void forwarded(unsigned before, gs_sparam_t *param)
{
	assert_int_equal(calls, before + 1);
	assert_ptr_equal(seen_param, param);
	assert_int_equal(logs, 0);
}

static void writes(void **state)
{
	(void)state;
	struct graphics_subsystem graphics = {0};
	gs_sparam_t *param = (gs_sparam_t *)&parameter_token;
	setup_graphics(&graphics);
	gs_enter_context(&graphics);
	expected_kind = 0;
	for (unsigned val = 0; val < 2; val++) {
		unsigned before = calls;
		gs_shader_set_bool(param, val != 0);
		forwarded(before, param);
		assert_int_equal(seen_bool, val != 0);
	}
	expected_kind = 1;
	const float floats[] = {0.0f, -0.0f, 1.25f, -42.5f, FLT_MAX, FLT_MIN, INFINITY, -INFINITY, NAN};
	for (unsigned i = 0; i < sizeof(floats) / sizeof(floats[0]); i++) {
		unsigned before = calls;
		gs_shader_set_float(param, floats[i]);
		forwarded(before, param);
		assert_memory_equal(&seen_float, &floats[i], sizeof(float));
	}
	expected_kind = 2;
	const int ints[] = {0, 1, -1, INT_MIN, INT_MAX, 1234567};
	for (unsigned i = 0; i < sizeof(ints) / sizeof(ints[0]); i++) {
		unsigned before = calls;
		gs_shader_set_int(param, ints[i]);
		forwarded(before, param);
		assert_int_equal(seen_int, ints[i]);
	}
	{
		struct matrix3 value = {0}, snapshot;
		/* Distinct bytes also detect any write through the const value pointer. */
		memset(&value, 4, sizeof(value));
		memcpy(&snapshot, &value, sizeof(value));
		expected_kind = 3;
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			unsigned before = calls;
			const struct matrix3 *input = repeat ? &snapshot : &value;
			gs_shader_set_matrix3(param, input);
			forwarded(before, param);
			assert_ptr_equal(seen_value, input);
			assert_memory_equal(&value, &snapshot, sizeof(value));
		}
	}
	{
		struct matrix4 value = {0}, snapshot;
		/* Distinct bytes also detect any write through the const value pointer. */
		memset(&value, 5, sizeof(value));
		memcpy(&snapshot, &value, sizeof(value));
		expected_kind = 4;
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			unsigned before = calls;
			const struct matrix4 *input = repeat ? &snapshot : &value;
			gs_shader_set_matrix4(param, input);
			forwarded(before, param);
			assert_ptr_equal(seen_value, input);
			assert_memory_equal(&value, &snapshot, sizeof(value));
		}
	}
	{
		struct vec2 value = {0}, snapshot;
		/* Distinct bytes also detect any write through the const value pointer. */
		memset(&value, 6, sizeof(value));
		memcpy(&snapshot, &value, sizeof(value));
		expected_kind = 5;
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			unsigned before = calls;
			const struct vec2 *input = repeat ? &snapshot : &value;
			gs_shader_set_vec2(param, input);
			forwarded(before, param);
			assert_ptr_equal(seen_value, input);
			assert_memory_equal(&value, &snapshot, sizeof(value));
		}
	}
	{
		struct vec3 value = {0}, snapshot;
		/* Distinct bytes also detect any write through the const value pointer. */
		memset(&value, 7, sizeof(value));
		memcpy(&snapshot, &value, sizeof(value));
		expected_kind = 6;
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			unsigned before = calls;
			const struct vec3 *input = repeat ? &snapshot : &value;
			gs_shader_set_vec3(param, input);
			forwarded(before, param);
			assert_ptr_equal(seen_value, input);
			assert_memory_equal(&value, &snapshot, sizeof(value));
		}
	}
	{
		struct vec4 value = {0}, snapshot;
		/* Distinct bytes also detect any write through the const value pointer. */
		memset(&value, 8, sizeof(value));
		memcpy(&snapshot, &value, sizeof(value));
		expected_kind = 7;
		for (unsigned repeat = 0; repeat < 2; repeat++) {
			unsigned before = calls;
			const struct vec4 *input = repeat ? &snapshot : &value;
			gs_shader_set_vec4(param, input);
			forwarded(before, param);
			assert_ptr_equal(seen_value, input);
			assert_memory_equal(&value, &snapshot, sizeof(value));
		}
	}
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_int_equal(graphics.ref, 1);
	gs_leave_context();
	teardown_graphics(&graphics);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(validation), cmocka_unit_test(writes)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
