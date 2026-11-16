/* Characterize real public timer wrappers without a GPU. The fake backend
 * owns the timer allocation; wrappers must only validate and dispatch. */
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
static gs_timer_t *expected_timer;
static uint64_t *expected_ticks;
static unsigned logs, events;
static int last_level;
static char last_log[160], order[32];
static bool backend_result, backend_write;
static uint64_t backend_ticks;

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

static void event(gs_timer_t *timer, char command)
{
	assert_ptr_equal(timer, expected_timer);
	assert_ptr_equal(gs_get_context(), &graphics);
	assert_true(events < sizeof(order) - 1);
	order[events++] = command;
	order[events] = 0;
}

static void backend_destroy(gs_timer_t *timer)
{
	event(timer, 'D');
	/* Only the backend releases the opaque allocation, exactly once. */
	bfree(timer);
	expected_timer = NULL;
}

static void backend_begin(gs_timer_t *timer)
{
	event(timer, 'B');
}

static void backend_end(gs_timer_t *timer)
{
	event(timer, 'E');
}

static bool backend_get_data(gs_timer_t *timer, uint64_t *ticks)
{
	event(timer, 'G');
	assert_ptr_equal(ticks, expected_ticks);
	if (backend_write)
		*ticks = backend_ticks;
	return backend_result;
}

static int setup(void **state)
{
	(void)state;
	memset(&graphics, 0, sizeof(graphics));
	memset(order, 0, sizeof(order));
	logs = events = 0;
	expected_timer = bmalloc(16);
	graphics.device = (gs_device_t *)&graphics;
	graphics.exports.device_enter_context = context;
	graphics.exports.device_leave_context = context;
	graphics.exports.gs_timer_destroy = backend_destroy;
	graphics.exports.gs_timer_begin = backend_begin;
	graphics.exports.gs_timer_end = backend_end;
	graphics.exports.gs_timer_get_data = backend_get_data;
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
	bfree(expected_timer);
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
	void (*commands[])(gs_timer_t *) = {gs_timer_destroy, gs_timer_begin, gs_timer_end};
	const char *names[] = {"gs_timer_destroy", "gs_timer_begin", "gs_timer_end"};
	for (unsigned i = 0; i < 3; i++) {
		commands[i](NULL);
		diagnostic(names[i], "called while not in a graphics context");
		commands[i](expected_timer);
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
	uint64_t ticks = UINT64_C(0xfedcba9876543210);
	for (unsigned active = 0; active < 2; active++) {
		if (active)
			gs_enter_context(&graphics);
		for (unsigned timer_present = 0; timer_present < 2; timer_present++) {
			for (unsigned ticks_present = 0; ticks_present < 2; ticks_present++) {
				if (active && timer_present && ticks_present)
					continue;
				assert_false(gs_timer_get_data(timer_present ? expected_timer : NULL,
							       ticks_present ? &ticks : NULL));
				/* Characterized, not endorsed: context masks both NULLs;
				 * timer validation masks ticks validation. No output reset. */
				diagnostic("gs_timer_get_data", !active ? "called while not in a graphics context"
								: !timer_present ? "Null 'timer' parameter"
										 : "Null 'ticks' parameter");
				assert_true(ticks == UINT64_C(0xfedcba9876543210));
			}
		}
	}
}

static void dispatch_and_ownership(void **state)
{
	(void)state;
	uint64_t ticks;
	const uint64_t values[] = {0, 1, UINT64_C(0xfedcba9876543210), UINT64_MAX};
	expected_ticks = &ticks;
	gs_enter_context(&graphics);
	/* Wrappers impose no begin/end state machine and do not defer calls. */
	gs_timer_end(expected_timer);
	assert_string_equal(order, "E");
	gs_timer_begin(expected_timer);
	gs_timer_begin(expected_timer);
	assert_string_equal(order, "EBB");
	gs_timer_end(expected_timer);
	assert_string_equal(order, "EBBE");
	for (unsigned result = 0; result < 2; result++) {
		backend_result = result != 0;
		for (unsigned write = 0; write < 2; write++) {
			backend_write = write != 0;
			for (unsigned i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
				ticks = 42;
				backend_ticks = values[i];
				unsigned before = events;
				assert_int_equal(gs_timer_get_data(expected_timer, &ticks), backend_result);
				assert_int_equal(events, before + 1);
				assert_int_equal(order[before], 'G');
				/* Even false may write; even true may leave output alone. */
				assert_true(ticks == (backend_write ? backend_ticks : 42));
			}
		}
	}
	gs_timer_destroy(expected_timer);
	assert_null(expected_timer);
	assert_string_equal(order, "EBBEGGGGGGGGGGGGGGGGD");
	assert_int_equal(logs, 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test_setup_teardown(command_guards, setup, teardown),
					   cmocka_unit_test_setup_teardown(data_guards, setup, teardown),
					   cmocka_unit_test_setup_teardown(dispatch_and_ownership, setup, teardown)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
