/* Direct characterization with the real private core layout and mutex.
 * No desktop session, background thread or portal is required. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <errno.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs-internal.h>

struct obs_core *hotkey_test_obs;
static struct obs_core core;
static obs_key_combination_t expected_combo;
static unsigned calls;
static bool callback_result;
static const char *callback_text;
static char *callback_storage;
static int wake_cookie;

/* The whole original hotkey source also contains the unrelated polling API.
 * Match the existing isolated hotkey fixtures: no platform input is pressed. */
bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	UNUSED_PARAMETER(context);
	UNUSED_PARAMETER(key);
	return false;
}

static bool shortcut_trigger(void *data, obs_key_combination_t combo, struct dstr *trigger)
{
	assert_ptr_equal(data, &wake_cookie);
	assert_int_equal(combo.key, expected_combo.key);
	assert_int_equal(combo.modifiers, expected_combo.modifiers);
	/* A nonrecursive fixture mutex exposes accidental unlock-before-dispatch. */
	assert_int_equal(pthread_mutex_trylock(&core.hotkeys.mutex), EBUSY);
	assert_null(trigger->array);
	assert_int_equal(trigger->len, 0);
	assert_int_equal(trigger->capacity, 0);
	calls++;
	if (callback_text) {
		/* Reserve even for an empty result: the returned allocation is owned. */
		dstr_reserve(trigger, 32);
		if (*callback_text)
			dstr_copy(trigger, callback_text);
		else
			trigger->array[0] = '\0';
	}
	callback_storage = trigger->array;
	return callback_result;
}

static char *query(void)
{
	char *result = obs_hotkey_combination_get_background_trigger(expected_combo);
	assert_int_equal(pthread_mutex_trylock(&core.hotkeys.mutex), 0);
	assert_int_equal(pthread_mutex_unlock(&core.hotkeys.mutex), 0);
	return result;
}

static void background_trigger(void **state)
{
	UNUSED_PARAMETER(state);
	expected_combo = (obs_key_combination_t){UINT32_MAX, OBS_KEY_A};
	assert_null(obs_hotkey_combination_get_background_trigger(expected_combo));
	assert_int_equal(calls, 0);

	assert_int_equal(pthread_mutex_init(&core.hotkeys.mutex, NULL), 0);
	hotkey_test_obs = &core;
	core.hotkeys.background.wake_data = &wake_cookie;
	assert_null(query());
	assert_int_equal(calls, 0);
	core.hotkeys.background.shortcut_trigger = shortcut_trigger;

	callback_result = false;
	callback_text = NULL;
	assert_null(query());
	assert_int_equal(calls, 1);
	callback_text = "allocated but rejected";
	long allocations = bnum_allocs();
	assert_null(query());
	assert_non_null(callback_storage);
	assert_int_equal(bnum_allocs(), allocations);
	assert_int_equal(calls, 2);

	callback_result = true;
	callback_text = "Ctrl+Alt+Shift+A (owned trigger)";
	char *result = query();
	assert_ptr_equal(result, callback_storage);
	assert_string_equal(result, callback_text);
	assert_int_equal(bnum_allocs(), allocations + 1);
	bfree(result);
	assert_int_equal(bnum_allocs(), allocations);

	expected_combo = (obs_key_combination_t){0, OBS_KEY_NONE};
	callback_text = "";
	result = query();
	assert_non_null(result);
	assert_ptr_equal(result, callback_storage);
	assert_string_equal(result, "");
	bfree(result);
	assert_int_equal(bnum_allocs(), allocations);

	/* A successful callback need not allocate: characterized, not endorsed. */
	callback_text = NULL;
	assert_null(query());
	assert_int_equal(calls, 5);
	hotkey_test_obs = NULL;
	assert_null(obs_hotkey_combination_get_background_trigger(expected_combo));
	assert_int_equal(calls, 5);
	assert_int_equal(pthread_mutex_destroy(&core.hotkeys.mutex), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(background_trigger)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
