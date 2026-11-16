/* Characterize the original C enumeration with an isolated core, without a
 * platform input thread. The library supplies the remaining hotkey dependencies
 * in both hybrid and C-reference runs. This pins the C source; the Rust swap must also exercise its exported symbol.
 * Keep this test unchanged for the swap. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <errno.h>
#include <string.h>
#include <obs-internal.h>

struct obs_core *hotkey_test_obs;
static struct obs_core core;

bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	(void)context;
	(void)key;
	return false;
}

static void *probe_mutex(void *data)
{
	int *result = data;
	*result = pthread_mutex_trylock(&core.hotkeys.mutex);
	if (*result == 0)
		pthread_mutex_unlock(&core.hotkeys.mutex);
	return NULL;
}

static void expect_mutex(int expected)
{
	pthread_t thread;
	int result = -1;
	assert_int_equal(pthread_create(&thread, NULL, probe_mutex, &result), 0);
	assert_int_equal(pthread_join(thread, NULL), 0);
	assert_int_equal(result, expected);
}

static int setup(void **state)
{
	(void)state;
	memset(&core, 0, sizeof(core));
	hotkey_test_obs = &core;
	assert_int_equal(pthread_mutex_init(&core.hotkeys.mutex, NULL), 0);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	assert_int_equal(pthread_mutex_destroy(&core.hotkeys.mutex), 0);
	hotkey_test_obs = NULL;
	return 0;
}

struct enumeration {
	struct obs_hotkey_background_shortcut *expected;
	size_t calls;
	size_t stop_after;
	bool change_live_list;
};

static bool collect(void *data, obs_key_combination_t combo, const char *trigger, bool bound)
{
	struct enumeration *record = data;
	size_t index = record->calls++;
	assert_true(index < core.hotkeys.background.shortcuts.num);
	struct obs_hotkey_background_shortcut *expected = &record->expected[index];
	assert_int_equal(combo.modifiers, expected->combo.modifiers);
	assert_int_equal(combo.key, expected->combo.key);
	assert_ptr_equal(trigger, expected->trigger);
	assert_int_equal(bound, expected->bound);
	expect_mutex(EBUSY);
	if (record->change_live_list && index == 0) {
		/* Characterized, not endorsed: later entries and num are read live,
		 * rather than snapshotting the list before invoking callbacks. */
		core.hotkeys.background.shortcuts.array = record->expected;
		core.hotkeys.background.shortcuts.num = 3;
	}
	return record->calls < record->stop_after;
}

static void no_core_and_empty_list(void **state)
{
	(void)state;
	struct enumeration record = {.stop_after = 99};
	hotkey_test_obs = NULL;
	obs_hotkeys_enum_background_shortcuts(NULL, &record);
	obs_hotkeys_enum_background_shortcuts(collect, &record);
	assert_int_equal(record.calls, 0);
	hotkey_test_obs = &core;
	obs_hotkeys_enum_background_shortcuts(collect, &record);
	assert_int_equal(record.calls, 0);
	expect_mutex(0);
}

static void null_callback_does_not_lock(void **state)
{
	(void)state;
	/* A nonrecursive mutex already held by this thread would deadlock if
	 * the NULL callback did not short-circuit before lock(). */
	assert_int_equal(pthread_mutex_lock(&core.hotkeys.mutex), 0);
	obs_hotkeys_enum_background_shortcuts(NULL, NULL);
	assert_int_equal(pthread_mutex_unlock(&core.hotkeys.mutex), 0);
	expect_mutex(0);
}

static void ordered_arguments_and_early_stop(void **state)
{
	(void)state;
	char bytes[] = {'x', (char)0xff, 0};
	char empty[] = "";
	struct obs_hotkey_background_shortcut shortcuts[] = {
		{{UINT32_MAX, OBS_KEY_NONE}, NULL, false},
		{{INTERACT_CONTROL_KEY, OBS_KEY_A}, bytes, true},
		{{INTERACT_CONTROL_KEY, OBS_KEY_A}, empty, false},
		{{0, OBS_KEY_MOUSE3}, bytes, true},
	};
	core.hotkeys.background.shortcuts.array = shortcuts;
	core.hotkeys.background.shortcuts.num = 4;
	for (size_t stop = 1; stop <= 5; stop++) {
		struct enumeration record = {.expected = shortcuts, .stop_after = stop};
		obs_hotkeys_enum_background_shortcuts(collect, &record);
		assert_int_equal(record.calls, stop < 4 ? stop : 4);
		expect_mutex(0);
		assert_int_equal(core.hotkeys.background.shortcuts.num, 4);
		assert_ptr_equal(core.hotkeys.background.shortcuts.array, shortcuts);
		assert_int_equal(bytes[0], 'x');
	}
}

static void list_is_read_live(void **state)
{
	(void)state;
	char first[] = "first";
	char second[] = "second";
	struct obs_hotkey_background_shortcut initial[] = {{{0, OBS_KEY_A}, first, false}};
	struct obs_hotkey_background_shortcut replacement[] = {
		{{0, OBS_KEY_A}, first, false},
		{{INTERACT_ALT_KEY, OBS_KEY_B}, second, true},
		{{INTERACT_SHIFT_KEY, OBS_KEY_C}, NULL, false},
	};
	core.hotkeys.background.shortcuts.array = initial;
	core.hotkeys.background.shortcuts.num = 1;
	struct enumeration record = {.expected = replacement, .stop_after = 99, .change_live_list = true};
	obs_hotkeys_enum_background_shortcuts(collect, &record);
	assert_int_equal(record.calls, 3);
	expect_mutex(0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(no_core_and_empty_list, setup, teardown),
		cmocka_unit_test_setup_teardown(null_callback_does_not_lock, setup, teardown),
		cmocka_unit_test_setup_teardown(ordered_arguments_and_early_stop, setup, teardown),
		cmocka_unit_test_setup_teardown(list_is_read_live, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
