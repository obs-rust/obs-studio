/* Characterize the native fallback through actual libobs, without a core. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <cmocka.h>
#include <obs.h>

static void native_fallback(void **state)
{
	(void)state;
	assert_false(obs_initialized());
	for (int key = OBS_KEY_NONE; key < OBS_KEY_LAST_VALUE; key++) {
		obs_key_combination_t combo = {0, (obs_key_t)key};
		assert_true(obs_hotkey_combination_is_background(combo));
		combo.modifiers = UINT32_MAX;
		assert_true(obs_hotkey_combination_is_background(combo));
	}
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(native_fallback)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
