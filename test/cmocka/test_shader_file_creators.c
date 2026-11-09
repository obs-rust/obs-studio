#include <setjmp.h>
#include <stdarg.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <inttypes.h>
#include <cmocka.h>

#include <graphics/graphics-internal.h>
#include <util/base.h>
#include <util/bmem.h>
#include <util/platform.h>

/* Characterize the public file wrappers before their Rust port (#4304).
 * The fake device records constructor dispatch; file reading, validation,
 * logging and freeing run through the real libobs. No GPU or display. */
static graphics_t *graphics;
static char device_token, shader_token;
static char error_token[] = "backend compile error";
static char source[256], log_text[512];
static const char *seen_file;
static char **seen_error;
static int vertex_calls, pixel_calls, log_level, log_calls;
static bool reject_shader;
static char directory[128], path[160];
static log_handler_t saved_handler;
static void *saved_param;

typedef gs_shader_t *(*file_creator_t)(const char *, char **);
static const file_creator_t creators[] = {gs_vertexshader_create_from_file, gs_pixelshader_create_from_file};
static const char *const names[] = {"gs_vertexshader_create_from_file", "gs_pixelshader_create_from_file"};
static const char *const kinds[] = {"vertex", "pixel"};

#if defined(__GNUC__) || defined(__clang__)
__attribute__((format(printf, 2, 0)))
#endif
static void capture_log(int level, const char *format, va_list args, void *param)
{
	(void)param;
	log_level = level;
	log_calls++;
	vsnprintf(log_text, sizeof(log_text), format, args);
}

static void context_noop(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static gs_shader_t *capture_shader(gs_device_t *device, const char *text, const char *file, char **error)
{
	assert_ptr_equal(device, &device_token);
	snprintf(source, sizeof(source), "%s", text);
	seen_file = file;
	seen_error = error;
	if (reject_shader) {
		if (error)
			*error = error_token;
		return NULL;
	}
	return (gs_shader_t *)&shader_token;
}

static gs_shader_t *vertex_create(gs_device_t *device, const char *text, const char *file, char **error)
{
	vertex_calls++;
	return capture_shader(device, text, file, error);
}

static gs_shader_t *pixel_create(gs_device_t *device, const char *text, const char *file, char **error)
{
	pixel_calls++;
	return capture_shader(device, text, file, error);
}

static void reset_recording(void)
{
	vertex_calls = pixel_calls = log_calls = 0;
	source[0] = log_text[0] = 0;
	seen_file = NULL;
	seen_error = NULL;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	graphics = bzalloc(sizeof(*graphics));
	assert_int_equal(pthread_mutex_init(&graphics->mutex, NULL), 0);
	graphics->device = (gs_device_t *)&device_token;
	graphics->exports.device_enter_context = context_noop;
	graphics->exports.device_leave_context = context_noop;
	graphics->exports.device_vertexshader_create = vertex_create;
	graphics->exports.device_pixelshader_create = pixel_create;
	base_get_log_handler(&saved_handler, &saved_param);
	base_set_log_handler(capture_log, NULL);
	snprintf(directory, sizeof(directory), "shader-file-%" PRIu64, os_gettime_ns());
	assert_int_equal(os_mkdir(directory), MKDIR_SUCCESS);
	snprintf(path, sizeof(path), "%s/source.shader", directory);
	reject_shader = false;
	reset_recording();
	gs_enter_context(graphics);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	base_set_log_handler(saved_handler, saved_param);
	pthread_mutex_destroy(&graphics->mutex);
	bfree(graphics);
	graphics = NULL;
	remove(path);
	assert_int_equal(os_rmdir(directory), 0);
	return 0;
}

static void context_and_path_gates(void **state)
{
	(void)state;
	gs_leave_context();
	for (size_t i = 0; i < 2; i++) {
		for (size_t null_path = 0; null_path < 2; null_path++) {
			char *error = error_token;
			char expected[256];
			reset_recording();
			assert_null(creators[i](null_path ? NULL : path, &error));
			snprintf(expected, sizeof(expected), "%s: called while not in a graphics context", names[i]);
			assert_string_equal(log_text, expected);
			assert_int_equal(log_level, LOG_DEBUG);
			assert_int_equal(log_calls, 1);
			assert_ptr_equal(error, error_token);
			assert_int_equal(vertex_calls + pixel_calls, 0);
		}
	}
	gs_enter_context(graphics);
	for (size_t i = 0; i < 2; i++) {
		char *error = error_token;
		char expected[256];
		reset_recording();
		assert_null(creators[i](NULL, &error));
		snprintf(expected, sizeof(expected), "%s: Null 'file' parameter", names[i]);
		assert_string_equal(log_text, expected);
		assert_int_equal(log_level, LOG_DEBUG);
		assert_int_equal(log_calls, 1);
		assert_ptr_equal(error, error_token);
		assert_int_equal(vertex_calls + pixel_calls, 0);
	}
}

static void unreadable_empty_and_bom_only_files(void **state)
{
	(void)state;
	/* Empty and BOM-only inputs are read failures, characterized, not endorsed. */
	for (size_t input = 0; input < 3; input++) {
		if (input)
			assert_true(os_quick_write_utf8_file(path, "", 0, input == 2));
		for (size_t i = 0; i < 2; i++) {
			char *error = error_token;
			char expected[256];
			reset_recording();
			assert_null(creators[i](path, &error));
			snprintf(expected, sizeof(expected), "Could not load %s shader file '%s'", kinds[i], path);
			assert_string_equal(log_text, expected);
			assert_int_equal(log_level, LOG_ERROR);
			assert_int_equal(log_calls, 1);
			assert_ptr_equal(error, error_token);
			assert_int_equal(vertex_calls + pixel_calls, 0);
			assert_null(creators[i](path, NULL));
		}
	}
}

static void utf8_dispatch_and_backend_results(void **state)
{
	(void)state;
	const char text[] = "// UTF-8: \xc3\xa9\nvoid main() {}\n";
	for (size_t bom = 0; bom < 2; bom++) {
		assert_true(os_quick_write_utf8_file(path, text, strlen(text), bom != 0));
		for (size_t i = 0; i < 2; i++) {
			for (size_t rejected = 0; rejected < 2; rejected++) {
				for (size_t no_error = 0; no_error < 2; no_error++) {
					char *error = NULL;
					char **output = no_error ? NULL : &error;
					reset_recording();
					reject_shader = rejected != 0;
					long allocations = bnum_allocs();
					gs_shader_t *result = creators[i](path, output);
					assert_ptr_equal(result, rejected ? NULL : &shader_token);
					assert_int_equal(bnum_allocs(), allocations);
					assert_string_equal(source, text);
					assert_ptr_equal(seen_file, path);
					assert_ptr_equal(seen_error, output);
					assert_ptr_equal(error, rejected && !no_error ? error_token : NULL);
					assert_int_equal(vertex_calls, i == 0 ? 1 : 0);
					assert_int_equal(pixel_calls, i == 1 ? 1 : 0);
					assert_int_equal(log_calls, 0);
				}
			}
		}
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(context_and_path_gates, setup, teardown),
		cmocka_unit_test_setup_teardown(unreadable_empty_and_bom_only_files, setup, teardown),
		cmocka_unit_test_setup_teardown(utf8_dispatch_and_backend_results, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
