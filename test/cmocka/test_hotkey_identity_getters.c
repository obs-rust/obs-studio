/* Actual-library characterization before the Rust swap. Stored null names
 * and arbitrary IDs are characterized, not normalized. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <obs-internal.h>

static void identity_fields(void **state)
{
	(void)state;
	struct obs_hotkey key = {0};
	char name[] = {'x', (char)0xff, 0};
	key.name = name;
	key.pressed = 3;
	key.pair_partner_id = 19;
	const obs_hotkey_id ids[] = {0, 1, 17, OBS_INVALID_HOTKEY_ID - 1, OBS_INVALID_HOTKEY_ID};
	for (size_t i = 0; i < sizeof(ids) / sizeof(ids[0]); i++) {
		key.id = ids[i];
		struct obs_hotkey before;
		memcpy(&before, &key, sizeof(key));
		assert_true(obs_hotkey_get_id(&key) == ids[i]);
		assert_ptr_equal(obs_hotkey_get_name(&key), name);
		assert_memory_equal(&key, &before, sizeof(key));
	}
	name[0] = 'y';
	assert_ptr_equal(obs_hotkey_get_name(&key), name);
	assert_int_equal(obs_hotkey_get_name(&key)[0], 'y');
	key.name = NULL;
	assert_null(obs_hotkey_get_name(&key));
	assert_true(obs_hotkey_get_id(&key) == OBS_INVALID_HOTKEY_ID);
	assert_int_equal(key.pressed, 3);
	assert_int_equal(key.pair_partner_id, 19);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(identity_fields)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
