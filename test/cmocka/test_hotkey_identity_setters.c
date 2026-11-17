/* Direct-library characterization: no hotkey implementation sources or fake core. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "obs.h"
#include "obs-hotkey.h"

static void callback(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	(void)data;
	(void)id;
	(void)key;
	(void)pressed;
}

static bool find(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	(void)id;
	*(obs_hotkey_t **)data = key;
	return false;
}

static void set_name_and_description(void **state)
{
	(void)state;
	obs_hotkey_id id = obs_hotkey_register_frontend("old.name", "Old", callback, NULL);
	assert_true(id != OBS_INVALID_HOTKEY_ID);
	obs_hotkey_t *key = NULL;
	obs_enum_hotkeys(find, &key);
	assert_non_null(key);
	obs_hotkey_set_name(id, "new.name");
	obs_hotkey_set_description(id, "New");
	assert_string_equal(obs_hotkey_get_name(key), "new.name");
	assert_string_equal(obs_hotkey_get_description(key), "New");
	obs_hotkey_set_name(id + 100, "x");
	obs_hotkey_set_description(OBS_INVALID_HOTKEY_ID, "x");
	assert_string_equal(obs_hotkey_get_name(key), "new.name");
	assert_string_equal(obs_hotkey_get_description(key), "New");
	/* Characterized, not endorsed: NULL clears each string. */
	obs_hotkey_set_name(id, NULL);
	obs_hotkey_set_description(id, NULL);
	assert_null(obs_hotkey_get_name(key));
	assert_null(obs_hotkey_get_description(key));
	obs_hotkey_set_name(id, "");
	obs_hotkey_set_description(id, "again");
	assert_string_equal(obs_hotkey_get_name(key), "");
	assert_string_equal(obs_hotkey_get_description(key), "again");
	obs_hotkey_unregister(id);
}

static int setup(void **state)
{
	(void)state;
	return obs_startup("en-US", NULL, NULL) ? 0 : -1;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(set_name_and_description)};
	return cmocka_run_group_tests(tests, setup, teardown);
}
