/* Characterize real public timer-range wrappers without a GPU. The fake
 * backend owns the allocation; wrappers only validate and dispatch. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <stdio.h>
#include <string.h>
#include "graphics/graphics-internal.h"
#include "util/base.h"
#include "util/bmem.h"

static struct graphics_subsystem graphics;
static gs_timer_range_t *allocation, *expected_range;
static bool *expected_disjoint;
static uint64_t *expected_frequency;
static unsigned logs, events;
static int last_level;
static char last_log[160], order[160];
static bool backend_result, backend_disjoint;
static unsigned backend_writes;
static uint64_t backend_frequency;

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

static void event(gs_timer_range_t *range, char command)
{
	assert_ptr_equal(range, expected_range);
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_true(events < sizeof(order) - 1);
	order[events++] = command;
	order[events] = 0;
}

static void backend_destroy(gs_timer_range_t *range)
{
	event(range, 'D');
	assert_ptr_equal(range, allocation);
	/* Only the backend releases the opaque allocation, exactly once. */
	bfree(range);
	allocation = expected_range = NULL;
}

static bool backend_begin(gs_timer_range_t *range)
{
	event(range, 'B');
	return backend_result;
}

static bool backend_end(gs_timer_range_t *range)
{
	event(range, 'E');
	return backend_result;
}

static bool backend_get_data(gs_timer_range_t *range, bool *disjoint, uint64_t *frequency)
{
	event(range, 'G');
	assert_ptr_equal(disjoint, expected_disjoint);
	assert_ptr_equal(frequency, expected_frequency);
	if (backend_writes & 1)
		*disjoint = backend_disjoint;
	if (backend_writes & 2)
		*frequency = backend_frequency;
	return backend_result;
}

static int setup(void **state)
{
	(void)state;
	memset(&graphics, 0, sizeof(graphics));
	memset(order, 0, sizeof(order));
	logs = events = 0;
	allocation = expected_range = bmalloc(16);
	graphics.device = (gs_device_t *)&graphics;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.gs_timer_range_destroy = backend_destroy;
	graphics.exports.gs_timer_range_begin = backend_begin;
	graphics.exports.gs_timer_range_end = backend_end;
	graphics.exports.gs_timer_range_get_data = backend_get_data;
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
	bfree(allocation);
	assert_int_equal(pthread_mutex_destroy(&graphics.mutex), 0);
	return 0;
}

static void diagnostic(const char *name, const char *reason)
{
	char expected[160];
	snprintf(expected, sizeof(expected), "%s: %s", name, reason);
	assert_int_equal(logs, 1);
	assert_int_equal(last_level, LOG_DEBUG);
	assert_string_equal(last_log, expected);
	assert_int_equal(events, 0);
	logs = 0;
}

static void command_guards(void **state)
{
	(void)state;
	void (*commands[])(gs_timer_range_t *) = {gs_timer_range_destroy, gs_timer_range_begin, gs_timer_range_end};
	const char *names[] = {"gs_timer_range_destroy", "gs_timer_range_begin", "gs_timer_range_end"};
	for (unsigned i = 0; i < 3; i++) {
		commands[i](NULL);
		diagnostic(names[i], "called while not in a graphics context");
		commands[i](allocation);
		diagnostic(names[i], "called while not in a graphics context");
	}
	gs_enter_context(&graphics);
	for (unsigned i = 0; i < 3; i++)
		commands[i](NULL);
	/* Characterized, not endorsed: NULL commands are silently ignored. */
	assert_int_equal(logs, 0);
	assert_int_equal(events, 0);
}

static void data_guards(void **state)
{
	(void)state;
	bool disjoint = true;
	uint64_t frequency = UINT64_C(0xfedcba9876543210);
	for (unsigned active = 0; active < 2; active++) {
		if (active)
			gs_enter_context(&graphics);
		for (unsigned range_present = 0; range_present < 2; range_present++) {
			for (unsigned outputs = 0; outputs < 4; outputs++) {
				if (active && outputs == 3)
					continue;
				assert_false(gs_timer_range_get_data(range_present ? allocation : NULL,
								     (outputs & 1) ? &disjoint : NULL,
								     (outputs & 2) ? &frequency : NULL));
				/* Characterized, not endorsed: context masks output NULLs;
				 * disjoint validation masks frequency. Range is not validated. */
				diagnostic("gs_timer_range_get_data", !active ? "called while not in a graphics context"
								      : !(outputs & 1) ? "Null 'disjoint' parameter"
										       : "Null 'frequency' parameter");
				assert_true(disjoint);
				assert_true(frequency == UINT64_C(0xfedcba9876543210));
			}
		}
	}
}

static void dispatch_and_ownership(void **state)
{
	(void)state;
	bool disjoint;
	uint64_t frequency;
	const uint64_t values[] = {0, 1, UINT64_C(0xfedcba9876543210), UINT64_MAX};
	expected_disjoint = &disjoint;
	expected_frequency = &frequency;
	gs_enter_context(&graphics);
	/* No begin/end state machine, deferral, or duplicate dispatch.
	 * Backend begin/end return values are discarded by the void public APIs. */
	backend_result = false;
	gs_timer_range_end(allocation);
	assert_string_equal(order, "E");
	backend_result = true;
	gs_timer_range_begin(allocation);
	backend_result = false;
	gs_timer_range_begin(allocation);
	assert_string_equal(order, "EBB");
	backend_result = true;
	gs_timer_range_end(allocation);
	assert_string_equal(order, "EBBE");
	for (unsigned range_present = 0; range_present < 2; range_present++) {
		expected_range = range_present ? allocation : NULL;
		for (unsigned result = 0; result < 2; result++) {
			backend_result = result != 0;
			for (backend_writes = 0; backend_writes < 4; backend_writes++) {
				for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
					backend_frequency = values[i];
					backend_disjoint = (i & 1) != 0;
					disjoint = !backend_disjoint;
					frequency = 42;
					unsigned before = events;
					/* Characterized, not endorsed: NULL range reaches the backend. */
					assert_int_equal(gs_timer_range_get_data(expected_range, &disjoint, &frequency),
							 backend_result);
					assert_int_equal(events, before + 1);
					assert_int_equal(order[before], 'G');
					/* Writes are independent of the result and of each other. */
					assert_int_equal(disjoint,
							 (backend_writes & 1) ? backend_disjoint : !backend_disjoint);
					assert_true(frequency == ((backend_writes & 2) ? backend_frequency : 42));
				}
			}
		}
	}
	assert_int_equal(events, 68);
	gs_timer_range_destroy(allocation);
	assert_null(allocation);
	assert_int_equal(events, 69);
	assert_int_equal(order[68], 'D');
	assert_int_equal(logs, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(command_guards, setup, teardown),
					   cmocka_unit_test_setup_teardown(data_guards, setup, teardown),
					   cmocka_unit_test_setup_teardown(dispatch_and_ownership, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
