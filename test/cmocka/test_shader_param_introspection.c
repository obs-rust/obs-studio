/* Public shader introspection characterization with a recording backend, no GPU.
 * Backend values and partial info writes are characterized, not endorsed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <limits.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

static struct graphics_subsystem graphics;
static unsigned char shader_token, param_token, result_token;
static gs_shader_t *shader;
static gs_sparam_t *param, *backend_result;
static const char *expected_name, *backend_name;
static uint32_t expected_index;
static struct gs_shader_param_info *expected_info;
static enum gs_shader_param_type backend_type;
static int backend_count, last_level;
static unsigned events, logs, info_writes;
static char last_log[160], order[64];

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *data)
{
	(void)data;
	logs++;
	last_level = level;
	vsnprintf(last_log, sizeof(last_log), format, args);
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, graphics.device);
}

static void event(char operation)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_true(events < sizeof(order) - 1);
	order[events++] = operation;
	order[events] = 0;
}

static int get_num_params(const gs_shader_t *value)
{
	assert_ptr_equal(value, shader);
	event('C');
	return backend_count;
}

static gs_sparam_t *get_by_idx(gs_shader_t *value, uint32_t index)
{
	assert_ptr_equal(value, shader);
	assert_int_equal(index, expected_index);
	event('I');
	return backend_result;
}

static gs_sparam_t *get_by_name(gs_shader_t *value, const char *name)
{
	assert_ptr_equal(value, shader);
	assert_ptr_equal(name, expected_name);
	event('N');
	return backend_result;
}

static void get_info(const gs_sparam_t *value, struct gs_shader_param_info *info)
{
	assert_ptr_equal(value, param);
	assert_ptr_equal(info, expected_info);
	event('O');
	if (info_writes & 1)
		info->type = backend_type;
	if (info_writes & 2)
		info->name = backend_name;
}

static int setup(void **state)
{
	(void)state;
	memset(&graphics, 0, sizeof(graphics));
	memset(order, 0, sizeof(order));
	events = logs = info_writes = 0;
	shader = (gs_shader_t *)&shader_token;
	param = (gs_sparam_t *)&param_token;
	backend_result = (gs_sparam_t *)&result_token;
	graphics.device = (gs_device_t *)&graphics;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.gs_shader_get_num_params = get_num_params;
	graphics.exports.gs_shader_get_param_by_idx = get_by_idx;
	graphics.exports.gs_shader_get_param_by_name = get_by_name;
	graphics.exports.gs_shader_get_param_info = get_info;
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

static void diagnostic(const char *function, const char *reason)
{
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: %s", function, reason);
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_DEBUG);
	assert_string_equal(last_log, expected);
	assert_int_equal(events, 0);
	logs = 0;
}

static void context_guards(void **state)
{
	(void)state;
	const char *reason = "called while not in a graphics context";
	const char sentinel[] = "untouched";
	struct gs_shader_param_info info = {GS_SHADER_PARAM_TEXTURE, sentinel};
	for (unsigned valid = 0; valid < 2; valid++) {
		gs_shader_t *s = valid ? shader : NULL;
		assert_int_equal(gs_shader_get_num_params(s), 0);
		diagnostic("gs_shader_get_num_params", reason);
		assert_null(gs_shader_get_param_by_idx(s, UINT32_MAX));
		diagnostic("gs_shader_get_param_by_idx", reason);
		for (unsigned named = 0; named < 2; named++) {
			assert_null(gs_shader_get_param_by_name(s, named ? sentinel : NULL));
			diagnostic("gs_shader_get_param_by_name", reason);
			gs_shader_get_param_info(valid ? param : NULL, named ? &info : NULL);
			diagnostic("gs_shader_get_param_info", reason);
			assert_int_equal(info.type, GS_SHADER_PARAM_TEXTURE);
			assert_ptr_equal(info.name, sentinel);
		}
	}
}

static void pointer_guards(void **state)
{
	(void)state;
	const char sentinel[] = "untouched";
	struct gs_shader_param_info info = {GS_SHADER_PARAM_VEC3, sentinel};
	gs_enter_context(&graphics);
	assert_int_equal(gs_shader_get_num_params(NULL), 0);
	diagnostic("gs_shader_get_num_params", "Null 'shader' parameter");
	assert_null(gs_shader_get_param_by_idx(NULL, 0));
	diagnostic("gs_shader_get_param_by_idx", "Null 'shader' parameter");
	assert_null(gs_shader_get_param_by_name(NULL, NULL));
	diagnostic("gs_shader_get_param_by_name", "Null 'shader' parameter");
	assert_null(gs_shader_get_param_by_name(NULL, sentinel));
	diagnostic("gs_shader_get_param_by_name", "Null 'shader' parameter");
	assert_null(gs_shader_get_param_by_name(shader, NULL));
	diagnostic("gs_shader_get_param_by_name", "Null 'name' parameter");
	gs_shader_get_param_info(NULL, NULL);
	diagnostic("gs_shader_get_param_info", "Null 'param' parameter");
	gs_shader_get_param_info(NULL, &info);
	diagnostic("gs_shader_get_param_info", "Null 'param' parameter");
	assert_int_equal(info.type, GS_SHADER_PARAM_VEC3);
	assert_ptr_equal(info.name, sentinel);
	gs_shader_get_param_info(param, NULL);
	diagnostic("gs_shader_get_param_info", "Null 'info' parameter");
}

static void dispatch_results(void **state)
{
	(void)state;
	const int counts[] = {0, 1, -1, INT_MIN, INT_MAX};
	const uint32_t indices[] = {0, 1, UINT32_MAX};
	char empty[] = "", name[] = "parameter[7].value";
	const char *names[] = {empty, name};
	gs_enter_context(&graphics);
	for (size_t i = 0; i < sizeof(counts) / sizeof(counts[0]); i++) {
		backend_count = counts[i];
		assert_int_equal(gs_shader_get_num_params(shader), counts[i]);
		assert_int_equal(events, i + 1);
	}
	assert_string_equal(order, "CCCCC");
	for (unsigned found = 0; found < 2; found++) {
		backend_result = found ? (gs_sparam_t *)&result_token : NULL;
		for (size_t i = 0; i < sizeof(indices) / sizeof(indices[0]); i++) {
			expected_index = indices[i];
			unsigned before = events;
			assert_ptr_equal(gs_shader_get_param_by_idx(shader, indices[i]), backend_result);
			assert_int_equal(events, before + 1);
		}
		for (size_t i = 0; i < sizeof(names) / sizeof(names[0]); i++) {
			expected_name = names[i];
			unsigned before = events;
			assert_ptr_equal(gs_shader_get_param_by_name(shader, names[i]), backend_result);
			assert_int_equal(events, before + 1);
		}
	}
	assert_string_equal(order, "CCCCCIIINNIIINN");
	assert_int_equal(logs, 0);
}

static void info_out_writes(void **state)
{
	(void)state;
	const char sentinel[] = "original", result[] = "backend name";
	gs_enter_context(&graphics);
	/* No initialization or repair by the wrapper: both, either or neither
	 * field can be written by the backend, including a NULL borrowed name. */
	for (info_writes = 0; info_writes < 4; info_writes++) {
		for (unsigned named = 0; named < 2; named++) {
			struct gs_shader_param_info info = {GS_SHADER_PARAM_INT, sentinel};
			backend_type = GS_SHADER_PARAM_UNKNOWN;
			backend_name = named ? result : NULL;
			expected_info = &info;
			unsigned before = events;
			gs_shader_get_param_info(param, &info);
			assert_int_equal(events, before + 1);
			assert_int_equal(info.type, (info_writes & 1) ? backend_type : GS_SHADER_PARAM_INT);
			assert_ptr_equal(info.name, (info_writes & 2) ? backend_name : sentinel);
		}
	}
	for (int type = GS_SHADER_PARAM_BOOL; type <= GS_SHADER_PARAM_TEXTURE; type++) {
		struct gs_shader_param_info info = {GS_SHADER_PARAM_UNKNOWN, NULL};
		backend_type = (enum gs_shader_param_type)type;
		backend_name = result;
		info_writes = 3;
		expected_info = &info;
		unsigned before = events;
		gs_shader_get_param_info(param, &info);
		assert_int_equal(events, before + 1);
		assert_int_equal(info.type, type);
		assert_ptr_equal(info.name, result);
	}
	assert_int_equal(logs, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(context_guards, setup, teardown),
					   cmocka_unit_test_setup_teardown(pointer_guards, setup, teardown),
					   cmocka_unit_test_setup_teardown(dispatch_results, setup, teardown),
					   cmocka_unit_test_setup_teardown(info_out_writes, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
