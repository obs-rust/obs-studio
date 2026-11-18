/* Characterize public indexbuffer maintenance through a GPU-free backend. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"

#if !__has_include("graphics/graphics-indexbuffer-maintenance.c.h")
#error "graphics indexbuffer maintenance extraction seam is missing"
#endif

enum maintenance_kind { DESTROY, FLUSH, DIRECT, COUNT };
static struct graphics_subsystem graphics;
static int device_token, buffer_token, data_token;
static unsigned call_count, log_count;
static enum maintenance_kind last_kind;
static const gs_indexbuffer_t *last_buffer;
static const void *last_data;
static size_t backend_count;
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

static void record(enum maintenance_kind kind, const gs_indexbuffer_t *buffer, const void *data)
{
	call_count++;
	last_kind = kind;
	last_buffer = buffer;
	last_data = data;
}

static void destroy(gs_indexbuffer_t *buffer)
{
	record(DESTROY, buffer, NULL);
}

static void flush(gs_indexbuffer_t *buffer)
{
	record(FLUSH, buffer, NULL);
}

static void direct(gs_indexbuffer_t *buffer, const void *data)
{
	record(DIRECT, buffer, data);
}

static size_t count(const gs_indexbuffer_t *buffer)
{
	record(COUNT, buffer, NULL);
	return backend_count;
}

static int setup(void **state)
{
	(void)state;
	assert_null(gs_get_context());
	memset(&graphics, 0, sizeof(graphics));
	call_count = log_count = 0;
	message[0] = '\0';
	last_buffer = NULL;
	last_data = NULL;
	backend_count = 0;
	graphics.device = (gs_device_t *)&device_token;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.gs_indexbuffer_destroy = destroy;
	graphics.exports.gs_indexbuffer_flush = flush;
	graphics.exports.gs_indexbuffer_flush_direct = direct;
	graphics.exports.gs_indexbuffer_get_num_indices = count;
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
	assert_int_equal(call_count, 0);
}

static void context_before_pointers(void **state)
{
	(void)state;
	gs_indexbuffer_t *buffer = (gs_indexbuffer_t *)&buffer_token;
	const char *reason = "called while not in a graphics context";
	for (unsigned nonnull = 0; nonnull < 2; nonnull++) {
		gs_indexbuffer_destroy(nonnull ? buffer : NULL);
		diagnostic("gs_indexbuffer_destroy", reason, nonnull * 4 + 1);
		gs_indexbuffer_flush(nonnull ? buffer : NULL);
		diagnostic("gs_indexbuffer_flush", reason, nonnull * 4 + 2);
		gs_indexbuffer_flush_direct(nonnull ? buffer : NULL, nonnull ? &data_token : NULL);
		diagnostic("gs_indexbuffer_flush_direct", reason, nonnull * 4 + 3);
		assert_true(gs_indexbuffer_get_num_indices(nonnull ? buffer : NULL) == 0);
		diagnostic("gs_indexbuffer_get_num_indices", reason, nonnull * 4 + 4);
	}
}

static void pointer_guards(void **state)
{
	(void)state;
	gs_enter_context(&graphics);
	/* Characterized, not endorsed: destroy silently accepts NULL in context. */
	gs_indexbuffer_destroy(NULL);
	assert_int_equal(log_count, 0);
	assert_int_equal(call_count, 0);
	gs_indexbuffer_flush(NULL);
	diagnostic("gs_indexbuffer_flush", "Null 'indexbuffer' parameter", 1);
	gs_indexbuffer_flush_direct(NULL, NULL);
	diagnostic("gs_indexbuffer_flush_direct", "Null 'indexbuffer' parameter", 2);
	gs_indexbuffer_flush_direct(NULL, &data_token);
	diagnostic("gs_indexbuffer_flush_direct", "Null 'indexbuffer' parameter", 3);
	gs_indexbuffer_flush_direct((gs_indexbuffer_t *)&buffer_token, NULL);
	diagnostic("gs_indexbuffer_flush_direct", "Null 'data' parameter", 4);
	assert_true(gs_indexbuffer_get_num_indices(NULL) == 0);
	diagnostic("gs_indexbuffer_get_num_indices", "Null 'indexbuffer' parameter", 5);
}

static void forwarded(unsigned expected_calls, enum maintenance_kind kind, const void *data)
{
	assert_int_equal(call_count, expected_calls);
	assert_int_equal(last_kind, kind);
	assert_ptr_equal(last_buffer, &buffer_token);
	assert_ptr_equal(last_data, data);
	assert_int_equal(log_count, 0);
}

static void backend_forwarding(void **state)
{
	(void)state;
	gs_indexbuffer_t *buffer = (gs_indexbuffer_t *)&buffer_token;
	const size_t counts[] = {0, 1, (size_t)UINT32_MAX, SIZE_MAX / 2 + 1, SIZE_MAX};
	gs_enter_context(&graphics);
	gs_indexbuffer_destroy(buffer);
	forwarded(1, DESTROY, NULL);
	gs_indexbuffer_flush(buffer);
	forwarded(2, FLUSH, NULL);
	gs_indexbuffer_flush_direct(buffer, &data_token);
	forwarded(3, DIRECT, &data_token);
	for (unsigned i = 0; i < sizeof(counts) / sizeof(counts[0]); i++) {
		backend_count = counts[i];
		/* Compare size_t directly, without narrowing through cmocka integers. */
		assert_true(gs_indexbuffer_get_num_indices(buffer) == counts[i]);
		forwarded(4 + i, COUNT, NULL);
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(context_before_pointers, setup, teardown),
		cmocka_unit_test_setup_teardown(pointer_guards, setup, teardown),
		cmocka_unit_test_setup_teardown(backend_forwarding, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
