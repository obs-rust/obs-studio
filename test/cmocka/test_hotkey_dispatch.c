/* C-source characterization; the external seam is reserved for the Rust swap.
 * Dispatch forwards events without binding lookup, locking or state changes.
 * A live core and (on a delivered path) a live hotkey/callback are preconditions:
 * NULL core/direct callback are unchecked C UB, characterized, not endorsed. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <string.h>
#include "libobs/build/libobs.pch.h"

static struct obs_core core;
static struct obs_core *current_core;
static obs_hotkey_t dispatch_hotkey;
static obs_hotkey_binding_t binding;
static int callback_data, router_data;
static unsigned callback_calls, router_calls;
static bool events[8];

static void record_callback(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	assert_ptr_equal(data, &callback_data);
	assert_int_equal(id, dispatch_hotkey.id);
	assert_ptr_equal(key, &dispatch_hotkey);
	assert_true(callback_calls < sizeof(events) / sizeof(events[0]));
	events[callback_calls++] = pressed;
}

static void record_router(void *data, obs_hotkey_id id, bool pressed)
{
	assert_ptr_equal(data, &router_data);
	assert_int_equal(id, dispatch_hotkey.id);
	assert_true(router_calls < sizeof(events) / sizeof(events[0]));
	events[router_calls++] = pressed;
}

#ifndef HOTKEY_DISPATCH_EXTERNAL
#ifndef HOTKEY_DISPATCH_SOURCE
#define HOTKEY_DISPATCH_SOURCE "libobs/obs-hotkey-dispatch.c.h"
#endif
#define obs current_core
#include HOTKEY_DISPATCH_SOURCE
#undef obs
#endif

static int setup(void **unused)
{
	(void)unused;
	memset(&core, 0, sizeof(core));
	memset(&dispatch_hotkey, 0, sizeof(dispatch_hotkey));
	memset(&binding, 0, sizeof(binding));
	memset(events, 0, sizeof(events));
	current_core = &core;
	dispatch_hotkey.id = 42;
	dispatch_hotkey.func = record_callback;
	dispatch_hotkey.data = &callback_data;
	binding.hotkey = &dispatch_hotkey;
	binding.hotkey_id = dispatch_hotkey.id;
	callback_calls = router_calls = 0;
	return 0;
}

static void active_binding_delivers_without_mutation(void **unused)
{
	(void)unused;
	binding.pressed = binding.modifiers_match = true;
	dispatch_hotkey.pressed = 2;
	core.hotkeys.bindings.array = &binding;
	core.hotkeys.bindings.num = core.hotkeys.bindings.capacity = 1;
	/* Even an installed router is ignored unless rerouting is enabled. */
	core.hotkeys.router_func = record_router;
	core.hotkeys.router_func_data = &router_data;
	obs_hotkey_dispatch(binding.hotkey, true);
	assert_int_equal(callback_calls, 1);
	assert_int_equal(router_calls, 0);
	assert_true(events[0]);
	assert_true(binding.pressed);
	assert_true(binding.modifiers_match);
	assert_int_equal(dispatch_hotkey.pressed, 2);
	assert_int_equal(core.hotkeys.bindings.num, 1);
	assert_ptr_equal(core.hotkeys.bindings.array, &binding);
}

static void absent_binding_still_delivers(void **unused)
{
	(void)unused;
	/* No registry or matching binding: dispatch performs no lookup. */
	dispatch_hotkey.id = OBS_INVALID_HOTKEY_ID;
	assert_null(core.hotkeys.hotkeys);
	assert_null(core.hotkeys.bindings.array);
	obs_hotkey_dispatch(&dispatch_hotkey, true);
	assert_int_equal(callback_calls, 1);
	assert_true(events[0]);
	assert_int_equal(dispatch_hotkey.pressed, 0);
	assert_false(binding.pressed);
	assert_null(core.hotkeys.bindings.array);
	/* An unrelated binding must not suppress delivery either. */
	binding.hotkey_id = 7;
	binding.hotkey = NULL;
	core.hotkeys.bindings.array = &binding;
	core.hotkeys.bindings.num = core.hotkeys.bindings.capacity = 1;
	obs_hotkey_dispatch(&dispatch_hotkey, false);
	assert_int_equal(callback_calls, 2);
	assert_false(events[1]);
	assert_int_equal(binding.hotkey_id, 7);
	assert_null(binding.hotkey);
}

static void press_release_and_duplicates_are_forwarded(void **unused)
{
	(void)unused;
	obs_hotkey_dispatch(&dispatch_hotkey, true);
	obs_hotkey_dispatch(&dispatch_hotkey, true);
	obs_hotkey_dispatch(&dispatch_hotkey, false);
	obs_hotkey_dispatch(&dispatch_hotkey, false);
	assert_int_equal(callback_calls, 4);
	assert_true(events[0]);
	assert_true(events[1]);
	assert_false(events[2]);
	assert_false(events[3]);
	assert_int_equal(dispatch_hotkey.pressed, 0);
	assert_false(binding.pressed);
}

static void routing_uses_current_router_and_id(void **unused)
{
	(void)unused;
	core.hotkeys.reroute_hotkeys = true;
	core.hotkeys.router_func = record_router;
	core.hotkeys.router_func_data = &router_data;
	/* Routed dispatch does not inspect the dispatch_hotkey's callback. */
	dispatch_hotkey.func = NULL;
	dispatch_hotkey.data = NULL;
	obs_hotkey_dispatch(&dispatch_hotkey, true);
	dispatch_hotkey.id = OBS_INVALID_HOTKEY_ID;
	obs_hotkey_dispatch(&dispatch_hotkey, false);
	assert_int_equal(router_calls, 2);
	assert_int_equal(callback_calls, 0);
	assert_true(events[0]);
	assert_false(events[1]);
	core.hotkeys.router_func = NULL;
	obs_hotkey_dispatch(&dispatch_hotkey, true);
	assert_int_equal(router_calls, 2);
	dispatch_hotkey.func = record_callback;
	dispatch_hotkey.data = &callback_data;
	core.hotkeys.reroute_hotkeys = false;
	obs_hotkey_dispatch(&dispatch_hotkey, false);
	assert_int_equal(callback_calls, 1);
	assert_false(events[0]);
}

static void zero_state_and_null_no_router_branch(void **unused)
{
	(void)unused;
	/* No startup or initialized mutex is needed by this function. */
	obs_hotkey_dispatch(&dispatch_hotkey, false);
	assert_int_equal(callback_calls, 1);
	assert_false(events[0]);
	core.hotkeys.reroute_hotkeys = true;
	/* This branch returns without touching even a NULL hotkey. */
	obs_hotkey_dispatch(NULL, true);
	obs_hotkey_dispatch(NULL, false);
	assert_int_equal(callback_calls, 1);
	assert_int_equal(router_calls, 0);
	assert_null(core.hotkeys.router_func);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup(active_binding_delivers_without_mutation, setup),
		cmocka_unit_test_setup(absent_binding_still_delivers, setup),
		cmocka_unit_test_setup(press_release_and_duplicates_are_forwarded, setup),
		cmocka_unit_test_setup(routing_uses_current_router_and_id, setup),
		cmocka_unit_test_setup(zero_state_and_null_no_router_branch, setup),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
