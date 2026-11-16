/* Characterize the actual library's background shortcut enumeration, so Tier 2
 * calls the symbol libobs exports (C reference or Rust) rather than a copy
 * compiled into the test, as test_hotkey_background_enum.c does. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs.h"
#include "obs-hotkey.h"
#include "util/threading.h"

struct enumeration {
	size_t calls;
	size_t stop_after;
};

static bool count(void *data, obs_key_combination_t combo, const char *trigger, bool bound)
{
	(void)combo;
	(void)trigger;
	(void)bound;
	struct enumeration *record = data;
	record->calls++;
	return record->calls < record->stop_after;
}

static size_t enumerate(size_t stop_after)
{
	struct enumeration record = {.stop_after = stop_after};
	obs_hotkeys_enum_background_shortcuts(count, &record);
	return record.calls;
}

static void *enumerate_thread(void *data)
{
	size_t *calls = data;
	*calls = enumerate(SIZE_MAX);
	return NULL;
}

static void background_enum_lifecycle(void **state)
{
	(void)state;
	/* No core: a NULL callback and a real one both return without calls. */
	obs_hotkeys_enum_background_shortcuts(NULL, NULL);
	assert_int_equal(enumerate(SIZE_MAX), 0);

	assert_true(obs_startup("en-US", NULL, NULL));
	obs_hotkeys_enum_background_shortcuts(NULL, NULL);
	size_t all = enumerate(SIZE_MAX);
	if (obs_hotkeys_get_background_state(NULL) == OBS_HOTKEYS_BACKGROUND_NATIVE)
		assert_int_equal(all, 0);
	assert_int_equal(enumerate(1), all ? 1 : 0);

	/* Every enumeration released the mutex: another thread can take it. */
	pthread_t thread;
	size_t thread_calls = SIZE_MAX;
	assert_int_equal(pthread_create(&thread, NULL, enumerate_thread, &thread_calls), 0);
	assert_int_equal(pthread_join(thread, NULL), 0);
	assert_int_equal(thread_calls, all);

	obs_shutdown();
	obs_hotkeys_enum_background_shortcuts(NULL, NULL);
	assert_int_equal(enumerate(SIZE_MAX), 0);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(background_enum_lifecycle)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
