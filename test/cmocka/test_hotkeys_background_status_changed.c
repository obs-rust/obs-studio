/* C-source characterization; the external seam is reserved for the Rust swap.
 * No desktop or threads: a recording hook checks synchronous signal delivery. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

static struct obs_core core;
static struct obs_core *current_core;
static signal_handler_t *first_handler, *second_handler;
static signal_handler_t *destinations[4];
static unsigned calls;
static bool reenter, remove_core;

static void record_signal(signal_handler_t *handler, const char *name, calldata_t *data)
{
	assert_true(calls < sizeof(destinations) / sizeof(destinations[0]));
	assert_string_equal(name, "hotkey_background_status_changed");
	assert_null(data);
	destinations[calls++] = handler;
	if (remove_core) {
		current_core = NULL;
	} else if (reenter) {
		reenter = false;
		core.hotkeys.signals = second_handler;
		obs_hotkeys_background_status_changed();
	}
}

#ifndef HOTKEY_BACKGROUND_STATUS_EXTERNAL
#ifndef HOTKEY_BACKGROUND_STATUS_SOURCE
#define HOTKEY_BACKGROUND_STATUS_SOURCE "libobs/obs-hotkeys-background-status-changed.c.h"
#endif
#define obs current_core
#define signal_handler_signal record_signal
#include HOTKEY_BACKGROUND_STATUS_SOURCE
#undef signal_handler_signal
#undef obs
#endif

static int setup(void **unused)
{
	(void)unused;
	memset(&core, 0, sizeof(core));
	memset(destinations, 0, sizeof(destinations));
	current_core = NULL;
	calls = 0;
	reenter = false;
	remove_core = false;
	first_handler = signal_handler_create();
	second_handler = signal_handler_create();
	assert_non_null(first_handler);
	assert_non_null(second_handler);
	return 0;
}

static int teardown(void **unused)
{
	(void)unused;
	signal_handler_destroy(first_handler);
	signal_handler_destroy(second_handler);
	return 0;
}

static void missing_core_or_handler_is_silent(void **unused)
{
	(void)unused;
	obs_hotkeys_background_status_changed();
	assert_int_equal(calls, 0);
	current_core = &core;
	obs_hotkeys_background_status_changed();
	assert_int_equal(calls, 0);
	assert_null(core.hotkeys.signals);
}

static void notification_uses_current_handler_each_time(void **unused)
{
	(void)unused;
	current_core = &core;
	core.hotkeys.signals = first_handler;
	obs_hotkeys_background_status_changed();
	obs_hotkeys_background_status_changed();
	assert_int_equal(calls, 2);
	assert_ptr_equal(destinations[0], first_handler);
	assert_ptr_equal(destinations[1], first_handler);
	assert_ptr_equal(current_core, &core);
	assert_ptr_equal(core.hotkeys.signals, first_handler);

	core.hotkeys.signals = second_handler;
	obs_hotkeys_background_status_changed();
	assert_int_equal(calls, 3);
	assert_ptr_equal(destinations[2], second_handler);
	core.hotkeys.signals = NULL;
	obs_hotkeys_background_status_changed();
	assert_int_equal(calls, 3);
}

static void synchronous_callback_can_reenter_or_remove_core(void **unused)
{
	(void)unused;
	current_core = &core;
	core.hotkeys.signals = first_handler;
	reenter = true;
	obs_hotkeys_background_status_changed();
	assert_int_equal(calls, 2);
	assert_ptr_equal(destinations[0], first_handler);
	assert_ptr_equal(destinations[1], second_handler);
	assert_false(reenter);
	assert_ptr_equal(core.hotkeys.signals, second_handler);

	remove_core = true;
	obs_hotkeys_background_status_changed();
	assert_int_equal(calls, 3);
	assert_ptr_equal(destinations[2], second_handler);
	assert_null(current_core);
	obs_hotkeys_background_status_changed();
	assert_int_equal(calls, 3);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(missing_core_or_handler_is_silent, setup, teardown),
		cmocka_unit_test_setup_teardown(notification_uses_current_handler_each_time, setup, teardown),
		cmocka_unit_test_setup_teardown(synchronous_callback_can_reenter_or_remove_core, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
