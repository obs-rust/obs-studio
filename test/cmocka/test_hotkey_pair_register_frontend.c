/* Direct characterization of the frontend pair registration public API.
 * The existing pair-latch test owns latch semantics; this pins registration,
 * argument ownership and routing through the actual shared libobs. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>

#include <obs.h>
#include <string.h>

struct observation {
	unsigned calls;
	unsigned callback;
	obs_hotkey_pair_id pair;
	obs_hotkey_t *key;
	bool pressed;
};

static bool record(void *data, obs_hotkey_pair_id pair, obs_hotkey_t *key, bool pressed, unsigned callback)
{
	struct observation *seen = data;
	seen->calls++;
	seen->callback = callback;
	seen->pair = pair;
	seen->key = key;
	seen->pressed = pressed;
	return false;
}

static bool first(void *data, obs_hotkey_pair_id pair, obs_hotkey_t *key, bool pressed)
{
	return record(data, pair, key, pressed, 1);
}

static bool second(void *data, obs_hotkey_pair_id pair, obs_hotkey_t *key, bool pressed)
{
	return record(data, pair, key, pressed, 2);
}

struct inventory {
	obs_hotkey_t *keys[8];
	size_t count;
};

static bool collect(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	struct inventory *keys = data;
	assert_true(keys->count < 8);
	assert_true(obs_hotkey_get_id(key) == id);
	keys->keys[keys->count++] = key;
	return true;
}

static struct inventory inventory(void)
{
	struct inventory keys = {0};
	obs_enum_hotkeys(collect, &keys);
	return keys;
}

static void unavailable_core(void **state)
{
	(void)state;
	assert_true(obs_hotkey_pair_register_frontend("first", "First", "second", "Second", first, second, NULL,
						      NULL) == OBS_INVALID_HOTKEY_PAIR_ID);
}

static void metadata_and_callbacks(void **state)
{
	(void)state;
	char name0[] = "frontend.first", description0[] = "First description";
	char name1[] = "frontend.second", description1[] = "Second description";
	struct observation seen[2] = {0};
	obs_hotkey_pair_id pair = obs_hotkey_pair_register_frontend(name0, description0, name1, description1, first,
								    second, &seen[0], &seen[1]);
	assert_true(pair != OBS_INVALID_HOTKEY_PAIR_ID);
	/* Registration owns copies of all four strings. */
	memset(name0, 'x', sizeof(name0) - 1);
	memset(name1, 'x', sizeof(name1) - 1);
	memset(description0, 'x', sizeof(description0) - 1);
	memset(description1, 'x', sizeof(description1) - 1);
	struct inventory keys = inventory();
	assert_int_equal(keys.count, 2);
	obs_hotkey_t *halves[2] = {NULL, NULL};
	for (size_t i = 0; i < keys.count; i++) {
		obs_hotkey_t *key = keys.keys[i];
		if (strcmp(obs_hotkey_get_name(key), "frontend.first") == 0)
			halves[0] = key;
		if (strcmp(obs_hotkey_get_name(key), "frontend.second") == 0)
			halves[1] = key;
		assert_int_equal(obs_hotkey_get_registerer_type(key), OBS_HOTKEY_REGISTERER_FRONTEND);
		assert_null(obs_hotkey_get_registerer(key));
	}
	assert_non_null(halves[0]);
	assert_non_null(halves[1]);
	assert_string_equal(obs_hotkey_get_description(halves[0]), "First description");
	assert_string_equal(obs_hotkey_get_description(halves[1]), "Second description");
	obs_hotkey_id ids[2] = {obs_hotkey_get_id(halves[0]), obs_hotkey_get_id(halves[1])};
	assert_true(ids[0] != OBS_INVALID_HOTKEY_ID);
	assert_true(ids[1] == ids[0] + 1);
	assert_true(obs_hotkey_get_pair_partner_id(halves[0]) == ids[1]);
	assert_true(obs_hotkey_get_pair_partner_id(halves[1]) == ids[0]);
	obs_hotkey_enable_callback_rerouting(true);
	for (size_t i = 0; i < 2; i++) {
		obs_hotkey_trigger_routed_callback(ids[i], true);
		assert_int_equal(seen[i].calls, 1);
		assert_int_equal(seen[i].callback, i + 1);
		assert_true(seen[i].pair == pair);
		assert_ptr_equal(seen[i].key, halves[i]);
		assert_true(seen[i].pressed);
		assert_int_equal(seen[1 - i].calls, i == 0 ? 0 : 2);
		obs_hotkey_trigger_routed_callback(ids[i], false);
		assert_int_equal(seen[i].calls, 2);
		assert_false(seen[i].pressed);
	}
	obs_hotkey_enable_callback_rerouting(false);
	obs_data_array_t *saved0 = NULL, *saved1 = NULL;
	obs_hotkey_pair_save(pair, &saved0, &saved1);
	assert_non_null(saved0);
	assert_non_null(saved1);
	assert_int_equal(obs_data_array_count(saved0), 0);
	assert_int_equal(obs_data_array_count(saved1), 0);
	obs_data_array_release(saved0);
	obs_data_array_release(saved1);
	obs_hotkey_pair_unregister(pair);
	assert_int_equal(inventory().count, 0);
}

static void repeated_and_empty_names(void **state)
{
	(void)state;
	struct observation seen[2] = {0};
	obs_hotkey_pair_id a = obs_hotkey_pair_register_frontend("", "", "", "", first, second, &seen[0], &seen[1]);
	obs_hotkey_pair_id b = obs_hotkey_pair_register_frontend("", "", "", "", first, second, &seen[0], &seen[1]);
	assert_true(a != OBS_INVALID_HOTKEY_PAIR_ID);
	assert_true(b == a + 1);
	/* Identical and empty names are legal, characterized, not endorsed. */
	struct inventory keys = inventory();
	assert_int_equal(keys.count, 4);
	for (size_t i = 0; i < keys.count; i++) {
		assert_string_equal(obs_hotkey_get_name(keys.keys[i]), "");
		assert_string_equal(obs_hotkey_get_description(keys.keys[i]), "");
	}
	obs_hotkey_pair_unregister(a);
	assert_int_equal(inventory().count, 2);
	obs_hotkey_pair_unregister(b);
	assert_int_equal(inventory().count, 0);
}

int main(void)
{
	const struct CMUnitTest absent[] = {cmocka_unit_test(unavailable_core)};
	int rc = cmocka_run_group_tests(absent, NULL, NULL);
	if (rc)
		return rc;
	if (!obs_startup("en-US", NULL, NULL))
		return 77;
	const struct CMUnitTest tests[] = {cmocka_unit_test(metadata_and_callbacks),
					   cmocka_unit_test(repeated_and_empty_names)};
	rc = cmocka_run_group_tests(tests, NULL, NULL);
	obs_shutdown();
	return rc;
}
