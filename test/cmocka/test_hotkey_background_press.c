/* Direct characterization of the private hotkey state and public signal.
 * No desktop session, portal or background thread is required. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs-internal.h>

struct obs_core *hotkey_test_obs;
static struct obs_core core;
static unsigned calls;
static bool expected_enabled;
static bool setter_active;
static int listener_cookie;

bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	UNUSED_PARAMETER(context);
	UNUSED_PARAMETER(key);
	return false;
}

static void background_press_changed(void *data, calldata_t *params)
{
	assert_ptr_equal(data, &listener_cookie);
	assert_true(setter_active);
	bool enabled = !expected_enabled;
	assert_true(calldata_get_bool(params, "enabled", &enabled));
	assert_int_equal(enabled, expected_enabled);
	assert_int_equal(core.hotkeys.thread_disable_press, !expected_enabled);
	/* The payload is fixed caller storage, valid throughout synchronous dispatch.
	 * Copy values only: neither params nor its stack survives the setter. */
	assert_true(params->fixed);
	assert_non_null(params->stack);
	assert_int_equal(params->capacity, 128);
	assert_true(params->size > 0);
	assert_true(params->size <= params->capacity);
	/* A nonrecursive mutex makes an accidental signal-under-lock observable. */
	assert_int_equal(pthread_mutex_trylock(&core.hotkeys.mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&core.hotkeys.mutex), 0);
	uint32_t flags = 0;
	assert_int_equal(obs_hotkeys_get_background_state(&flags), OBS_HOTKEYS_BACKGROUND_NATIVE);
	assert_int_equal(flags, 0x1234);
	calls++;
}

static void set_background_press(bool enabled, unsigned expected_calls)
{
	expected_enabled = enabled;
	setter_active = true;
	obs_hotkey_enable_background_press(enabled);
	setter_active = false;
	assert_int_equal(calls, expected_calls);
	assert_int_equal(core.hotkeys.thread_disable_press, !enabled);
	assert_int_equal(pthread_mutex_trylock(&core.hotkeys.mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&core.hotkeys.mutex), 0);
}

static void background_press(void **state)
{
	UNUSED_PARAMETER(state);
	/* No core means no mutex or signal access, for either argument. */
	obs_hotkey_enable_background_press(false);
	obs_hotkey_enable_background_press(true);
	assert_int_equal(calls, 0);

	long allocations = bnum_allocs();
	assert_int_equal(pthread_mutex_init(&core.hotkeys.mutex, NULL), 0);
	core.hotkeys.signals = signal_handler_create();
	assert_non_null(core.hotkeys.signals);
	assert_true(signal_handler_add(core.hotkeys.signals, "void hotkey_background_press_changed(bool enabled)"));
	signal_handler_connect(core.hotkeys.signals, "hotkey_background_press_changed", background_press_changed,
			       &listener_cookie);
	core.hotkeys.background.state = OBS_HOTKEYS_BACKGROUND_NATIVE;
	core.hotkeys.background.flags = 0x1234;
	hotkey_test_obs = &core;

	/* Zero-initialized disable=false starts logically enabled. */
	set_background_press(true, 0);
	set_background_press(false, 1);
	set_background_press(false, 1);
	set_background_press(true, 2);
	set_background_press(true, 2);
	set_background_press(false, 3);
	set_background_press(true, 4);

	hotkey_test_obs = NULL;
	obs_hotkey_enable_background_press(false);
	assert_false(core.hotkeys.thread_disable_press);
	assert_int_equal(calls, 4);
	signal_handler_disconnect(core.hotkeys.signals, "hotkey_background_press_changed", background_press_changed,
				  &listener_cookie);
	signal_handler_destroy(core.hotkeys.signals);
	assert_int_equal(pthread_mutex_destroy(&core.hotkeys.mutex), 0);
	assert_int_equal(bnum_allocs(), allocations);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(background_press)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
