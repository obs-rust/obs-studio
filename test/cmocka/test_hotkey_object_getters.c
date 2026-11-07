/* Characterization through actual libobs symbols, before the Rust swap.
 * Stored nulls and independent fields are characterized, not normalized. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs-internal.h>

static void object_fields(void **state)
{
	(void)state;
	struct obs_hotkey key = {0};
	char description[] = {'x', (char)0xff, 0};
	int token = 17;
	key.description = description;
	key.registerer = &token;
	key.pair_partner_id = OBS_INVALID_HOTKEY_ID;
	key.id = 19;
	key.pressed = 3;
	for (int kind = OBS_HOTKEY_REGISTERER_FRONTEND; kind <= OBS_HOTKEY_REGISTERER_SERVICE; kind++) {
		key.registerer_type = (obs_hotkey_registerer_t)kind;
		assert_ptr_equal(obs_hotkey_get_description(&key), description);
		assert_int_equal(obs_hotkey_get_registerer_type(&key), kind);
		assert_ptr_equal(obs_hotkey_get_registerer(&key), &token);
		assert_int_equal(obs_hotkey_get_pair_partner_id(&key), OBS_INVALID_HOTKEY_ID);
		assert_int_equal(key.id, 19);
		assert_int_equal(key.pressed, 3);
	}
	description[0] = 'y';
	assert_ptr_equal(obs_hotkey_get_description(&key), description);
	assert_int_equal(obs_hotkey_get_description(&key)[0], 'y');
	key.description = NULL;
	key.registerer = NULL;
	key.pair_partner_id = 0;
	assert_null(obs_hotkey_get_description(&key));
	assert_null(obs_hotkey_get_registerer(&key));
	assert_int_equal(obs_hotkey_get_pair_partner_id(&key), 0);
	assert_int_equal(key.id, 19);
	assert_int_equal(key.pressed, 3);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(object_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
