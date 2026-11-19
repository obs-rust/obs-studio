/* Characterize index-buffer data/type queries through a GPU-free backend. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

/* This characterization lands with the extraction, before the Rust swap. */
#if !__has_include("graphics/graphics-indexbuffer-queries.c.h")
#error "graphics indexbuffer query extraction seam is missing"
#endif

static struct graphics_subsystem graphics;
static int device_token, buffer_tokens[2], data_tokens[2];
static const gs_indexbuffer_t *expected_buffer;
static void *backend_data;
static enum gs_index_type backend_type;
static unsigned data_calls, type_calls, log_count;
static char message[160];
static log_handler_t previous_handler;
static void *previous_param;

#ifdef __GNUC__
__attribute__((format(printf, 2, 0)))
#endif
static void capture(int level, const char *format, va_list args, void *param)
{
	(void)param;
	assert_int_equal(level, LOG_DEBUG);
	log_count++;
	vsnprintf(message, sizeof(message), format, args);
}

static void context(gs_device_t *device)
{
	assert_ptr_equal(device, &device_token);
}

static void *get_data(const gs_indexbuffer_t *buffer)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(buffer, expected_buffer);
	data_calls++;
	return backend_data;
}

static enum gs_index_type get_type(const gs_indexbuffer_t *buffer)
{
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_ptr_equal(buffer, expected_buffer);
	type_calls++;
	return backend_type;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	data_calls = type_calls = log_count = 0;
	message[0] = '\0';
	expected_buffer = NULL;
	backend_data = NULL;
	backend_type = GS_UNSIGNED_SHORT;
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.gs_indexbuffer_get_data = get_data;
	graphics.exports.gs_indexbuffer_get_type = get_type;
	assert_int_equal(pthread_mutex_init(&graphics.mutex, NULL), 0);
	base_get_log_handler(&previous_handler, &previous_param);
	base_set_log_handler(capture, NULL);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	if (gs_get_context())
		gs_leave_context();
	base_set_log_handler(previous_handler, previous_param);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void diagnostic(const char *name, const char *reason, unsigned expected_logs)
{
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: %s", name, reason);
	assert_string_equal(message, expected);
	assert_int_equal(log_count, expected_logs);
	assert_int_equal(data_calls, 0);
	assert_int_equal(type_calls, 0);
}

static void context_before_pointers(void **state)
{
	(void)state;
	const gs_indexbuffer_t *buffer = (const gs_indexbuffer_t *)&buffer_tokens[0];
	for (unsigned nonnull = 0; nonnull < 2; nonnull++) {
		assert_null(gs_indexbuffer_get_data(nonnull ? buffer : NULL));
		diagnostic("gs_indexbuffer_get_data", "called while not in a graphics context", nonnull * 2 + 1);
		/* Characterized, not endorsed: the fallback is the C enum cast zero. */
		assert_int_equal(gs_indexbuffer_get_type(nonnull ? buffer : NULL), (enum gs_index_type)0);
		diagnostic("gs_indexbuffer_get_type", "called while not in a graphics context", nonnull * 2 + 2);
	}
}

static void pointer_guards(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	assert_null(gs_indexbuffer_get_data(NULL));
	diagnostic("gs_indexbuffer_get_data", "Null 'indexbuffer' parameter", 1);
	assert_int_equal(gs_indexbuffer_get_type(NULL), (enum gs_index_type)0);
	diagnostic("gs_indexbuffer_get_type", "Null 'indexbuffer' parameter", 2);
}

static void backend_results_and_buffer_routing(void **state)
{
	(void)state;
	const enum gs_index_type types[] = {GS_UNSIGNED_SHORT, GS_UNSIGNED_LONG};
	void *const data[] = {NULL, &data_tokens[0], &data_tokens[1]};
	gs_enter_context(&graphics);
	for (unsigned buffer = 0; buffer < 2; buffer++) {
		expected_buffer = (const gs_indexbuffer_t *)&buffer_tokens[buffer];
		for (unsigned i = 0; i < sizeof(data) / sizeof(data[0]); i++) {
			backend_data = data[i];
			unsigned previous_types = type_calls;
			unsigned previous_data = data_calls;
			/* A NULL backend result is successful forwarding, without a diagnostic. */
			assert_ptr_equal(gs_indexbuffer_get_data(expected_buffer), data[i]);
			assert_int_equal(data_calls, previous_data + 1);
			assert_int_equal(type_calls, previous_types);
			assert_int_equal(log_count, 0);
		}
		for (unsigned i = 0; i < sizeof(types) / sizeof(types[0]); i++) {
			backend_type = types[i];
			unsigned previous_data = data_calls;
			unsigned previous_types = type_calls;
			assert_int_equal(gs_indexbuffer_get_type(expected_buffer), types[i]);
			assert_int_equal(type_calls, previous_types + 1);
			assert_int_equal(data_calls, previous_data);
			assert_int_equal(log_count, 0);
		}
	}
	assert_int_equal(data_calls, 6);
	assert_int_equal(type_calls, 4);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(context_before_pointers, setup, teardown),
		cmocka_unit_test_setup_teardown(pointer_guards, setup, teardown),
		cmocka_unit_test_setup_teardown(backend_results_and_buffer_routing, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
