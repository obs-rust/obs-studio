/* Characterize the actual libobs entry points, not a source-local copy.
 * The getters read independent fields: they do not resolve the binding ID,
 * dereference the stored hotkey, normalize modifiers, or update latches. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs-internal.h>

static void binding_fields(void **state)
{
	(void)state;
	struct obs_hotkey hotkey = {0};
	hotkey.id = 17;
	struct obs_hotkey_binding binding = {0};
	binding.key.modifiers = UINT32_MAX;
	binding.key.key = OBS_KEY_A;
	binding.pressed = true;
	binding.modifiers_match = true;
	binding.hotkey_id = OBS_INVALID_HOTKEY_ID;
	binding.hotkey = &hotkey;
	obs_key_combination_t key = obs_hotkey_binding_get_key_combination(&binding);
	assert_int_equal(key.modifiers, UINT32_MAX);
	assert_int_equal(key.key, OBS_KEY_A);
	/* Characterized, not endorsed: the binding ID need not match hotkey->id. */
	assert_int_equal(obs_hotkey_binding_get_hotkey_id(&binding), OBS_INVALID_HOTKEY_ID);
	assert_ptr_equal(obs_hotkey_binding_get_hotkey(&binding), &hotkey);
	assert_true(binding.pressed);
	assert_true(binding.modifiers_match);
	binding.key.modifiers = 0;
	binding.key.key = OBS_KEY_NONE;
	binding.hotkey_id = 0;
	binding.hotkey = NULL;
	key = obs_hotkey_binding_get_key_combination(&binding);
	assert_int_equal(key.modifiers, 0);
	assert_int_equal(key.key, OBS_KEY_NONE);
	assert_int_equal(obs_hotkey_binding_get_hotkey_id(&binding), 0);
	assert_null(obs_hotkey_binding_get_hotkey(&binding));
	assert_true(binding.pressed);
	assert_true(binding.modifiers_match);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(binding_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
