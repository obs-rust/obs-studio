/* Characterize the real C query and store against one isolated core, without
 * starting OBS or a desktop. The graph keeps both functions in the same core
 * unity group; the later Rust swap must preserve this callable boundary. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "libobs/build/libobs.pch.h"

static struct obs_core core;
struct obs_core *hotkey_test_obs;
static long allocations;

bool obs_hotkeys_platform_is_pressed(obs_hotkeys_platform_t *context, obs_key_t key)
{
	(void)context;
	(void)key;
	return false;
}

static int setup(void **unused)
{
	(void)unused;
	memset(&core, 0, sizeof(core));
	hotkey_test_obs = &core;
	allocations = bnum_allocs();
	return 0;
}

static int teardown(void **unused)
{
	(void)unused;
	/* Match obs_hotkeys_free: each translation is owned by the C allocator. */
	for (size_t i = 0; i < OBS_KEY_LAST_VALUE; i++) {
		bfree(core.hotkeys.translations[i]);
		core.hotkeys.translations[i] = NULL;
	}
	hotkey_test_obs = NULL;
	assert_int_equal(bnum_allocs(), allocations);
	return 0;
}

static void none_without_core(void **unused)
{
	(void)unused;
	char fallback[] = "default";
	hotkey_test_obs = NULL;
	assert_null(obs_get_hotkey_translation(OBS_KEY_NONE, fallback));
	assert_null(obs_get_hotkey_translation(OBS_KEY_NONE, NULL));
}

static void stored_pointer(void **unused)
{
	(void)unused;
	char translation[] = "translated A";
	char fallback[] = "default";
	obs_hotkeys_store_key_translation(OBS_KEY_A, translation);
	const char *stored = core.hotkeys.translations[OBS_KEY_A];
	assert_non_null(stored);
	assert_ptr_not_equal(stored, translation);
	assert_string_equal(stored, translation);
	assert_ptr_equal(obs_get_hotkey_translation(OBS_KEY_A, fallback), stored);
	assert_ptr_equal(obs_get_hotkey_translation(OBS_KEY_A, NULL), stored);
	assert_int_equal(bnum_allocs(), allocations + 1);
}

static void unset_fallback(void **unused)
{
	(void)unused;
	char fallback[] = "default";
	char empty[] = "";
	assert_ptr_equal(obs_get_hotkey_translation(OBS_KEY_B, fallback), fallback);
	assert_ptr_equal(obs_get_hotkey_translation(OBS_KEY_B, empty), empty);
	assert_null(core.hotkeys.translations[OBS_KEY_B]);
}

static void null_default(void **unused)
{
	(void)unused;
	assert_null(obs_get_hotkey_translation(OBS_KEY_B, NULL));
}

static void restore_pointer(void **unused)
{
	(void)unused;
	char first[] = "first";
	char second[] = "a longer replacement translation";
	char empty[] = "";
	char fallback[] = "default";
	const char *values[] = {first, second, empty};
	for (size_t i = 0; i < sizeof(values) / sizeof(values[0]); i++) {
		obs_hotkeys_store_key_translation(OBS_KEY_A, values[i]);
		const char *stored = core.hotkeys.translations[OBS_KEY_A];
		assert_non_null(stored);
		assert_ptr_not_equal(stored, values[i]);
		assert_string_equal(stored, values[i]);
		/* An allocator may reuse the old address: compare with the CURRENT slot. */
		assert_ptr_equal(obs_get_hotkey_translation(OBS_KEY_A, fallback), stored);
		assert_int_equal(bnum_allocs(), allocations + 1);
	}
	obs_hotkeys_store_key_translation(OBS_KEY_A, NULL);
	assert_null(core.hotkeys.translations[OBS_KEY_A]);
	assert_ptr_equal(obs_get_hotkey_translation(OBS_KEY_A, fallback), fallback);
	assert_null(obs_get_hotkey_translation(OBS_KEY_A, NULL));
	assert_int_equal(bnum_allocs(), allocations);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(none_without_core, setup, teardown),
		cmocka_unit_test_setup_teardown(stored_pointer, setup, teardown),
		cmocka_unit_test_setup_teardown(unset_fallback, setup, teardown),
		cmocka_unit_test_setup_teardown(null_default, setup, teardown),
		cmocka_unit_test_setup_teardown(restore_pointer, setup, teardown),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
