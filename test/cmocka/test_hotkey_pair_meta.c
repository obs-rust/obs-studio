/* Actual-library characterization, shared by hybrid and C-reference libobs.
 * The setters require an initialized core. NULL core and inputs aliasing the
 * hotkey-owned string being freed are outside the characterized valid domain. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include "cmocka-compat.h"
#include <string.h>
#include <obs.h>
#include <obs-hotkey.h>

struct meta_fixture {
	obs_hotkey_pair_id pair;
	obs_hotkey_pair_id other;
	obs_hotkey_t *keys[2];
	obs_hotkey_t *other_keys[2];
	obs_hotkey_t *single;
	size_t signals;
};

struct meta_find {
	const char *name;
	obs_hotkey_t *key;
};

static bool find_named_key(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	(void)id;
	struct meta_find *find = data;
	const char *name = obs_hotkey_get_name(key);
	if (name && strcmp(name, find->name) == 0)
		find->key = key;
	return true;
}

static obs_hotkey_t *named_key(const char *name)
{
	struct meta_find find = {name, NULL};
	obs_enum_hotkeys(find_named_key, &find);
	assert_non_null(find.key);
	return find.key;
}

static void no_hotkey(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	(void)data;
	(void)id;
	(void)key;
	(void)pressed;
}

static bool no_pair(void *data, obs_hotkey_pair_id id, obs_hotkey_t *key, bool pressed)
{
	(void)data;
	(void)id;
	(void)key;
	(void)pressed;
	return false;
}

static void count_metadata_signals(void *data, const char *signal, calldata_t *params)
{
	(void)params;
	struct meta_fixture *fixture = data;
	/* Observe every hotkey signal except the independent background service's
	 * asynchronous notifications. No clock or hotkey-thread polling is used. */
	if (strncmp(signal, "hotkey_", 7) == 0 && strncmp(signal, "hotkey_background_", 18) != 0)
		fixture->signals++;
}

static void check_text(const char *actual, const char *expected)
{
	if (expected) {
		assert_non_null(actual);
		assert_string_equal(actual, expected);
	} else {
		assert_null(actual);
	}
}

static void check_pair(struct meta_fixture *fixture, const char *name0, const char *name1, const char *desc0,
		       const char *desc1)
{
	check_text(obs_hotkey_get_name(fixture->keys[0]), name0);
	check_text(obs_hotkey_get_name(fixture->keys[1]), name1);
	check_text(obs_hotkey_get_description(fixture->keys[0]), desc0);
	check_text(obs_hotkey_get_description(fixture->keys[1]), desc1);
	check_text(obs_hotkey_get_name(fixture->other_keys[0]), "other.0");
	check_text(obs_hotkey_get_name(fixture->other_keys[1]), "other.1");
	check_text(obs_hotkey_get_description(fixture->other_keys[0]), "Other 0");
	check_text(obs_hotkey_get_description(fixture->other_keys[1]), "Other 1");
	check_text(obs_hotkey_get_name(fixture->single), "single");
	check_text(obs_hotkey_get_description(fixture->single), "Single");
	assert_int_equal(fixture->signals, 0);
}

static void pair_metadata_contract(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	struct meta_fixture fixture = {0};
	signal_handler_t *signals = obs_get_signal_handler();
	signal_handler_connect_global(signals, count_metadata_signals, &fixture);

	/* Unknown IDs are no-ops even with an empty pair registry. */
	obs_hotkey_pair_set_names(0, NULL, "unused");
	obs_hotkey_pair_set_descriptions(OBS_INVALID_HOTKEY_PAIR_ID, "unused", NULL);
	assert_int_equal(fixture.signals, 0);

	obs_hotkey_id single_id = obs_hotkey_register_frontend("single", "Single", no_hotkey, NULL);
	assert_true(single_id != OBS_INVALID_HOTKEY_ID);
	fixture.other = obs_hotkey_pair_register_frontend("other.0", "Other 0", "other.1", "Other 1", no_pair, no_pair,
							  NULL, NULL);
	fixture.pair =
		obs_hotkey_pair_register_frontend("pair.0", "Pair 0", "pair.1", "Pair 1", no_pair, no_pair, NULL, NULL);
	assert_true(fixture.other != OBS_INVALID_HOTKEY_PAIR_ID);
	assert_true(fixture.pair != OBS_INVALID_HOTKEY_PAIR_ID);
	/* Positive control: our observer sees all five registration signals. */
	assert_int_equal(fixture.signals, 5);
	fixture.signals = 0;
	fixture.single = named_key("single");
	fixture.other_keys[0] = named_key("other.0");
	fixture.other_keys[1] = named_key("other.1");
	fixture.keys[0] = named_key("pair.0");
	fixture.keys[1] = named_key("pair.1");
	obs_hotkey_id ids[] = {obs_hotkey_get_id(fixture.keys[0]), obs_hotkey_get_id(fixture.keys[1])};
	assert_true(fixture.pair != ids[0] && fixture.pair != ids[1]);
	assert_int_equal(obs_hotkey_get_pair_partner_id(fixture.keys[0]), ids[1]);
	assert_int_equal(obs_hotkey_get_pair_partner_id(fixture.keys[1]), ids[0]);

	obs_hotkey_pair_id unknown[] = {fixture.pair + 1, OBS_INVALID_HOTKEY_PAIR_ID};
	for (size_t i = 0; i < 2; i++) {
		obs_hotkey_pair_set_names(unknown[i], "wrong.0", "wrong.1");
		obs_hotkey_pair_set_descriptions(unknown[i], NULL, NULL);
		check_pair(&fixture, "pair.0", "pair.1", "Pair 0", "Pair 1");
	}

	char name0[] = "new.0", name1[] = "new.1";
	char desc0[] = "New 0", desc1[] = "New 1";
	const char *old_desc0 = obs_hotkey_get_description(fixture.keys[0]);
	const char *old_desc1 = obs_hotkey_get_description(fixture.keys[1]);
	obs_hotkey_pair_set_names(fixture.pair, name0, name1);
	check_pair(&fixture, "new.0", "new.1", "Pair 0", "Pair 1");
	assert_ptr_equal(obs_hotkey_get_description(fixture.keys[0]), old_desc0);
	assert_ptr_equal(obs_hotkey_get_description(fixture.keys[1]), old_desc1);
	assert_ptr_not_equal(obs_hotkey_get_name(fixture.keys[0]), name0);
	assert_ptr_not_equal(obs_hotkey_get_name(fixture.keys[1]), name1);
	name0[0] = 'X';
	name1[0] = 'Y';
	check_pair(&fixture, "new.0", "new.1", "Pair 0", "Pair 1");

	const char *saved_name0 = obs_hotkey_get_name(fixture.keys[0]);
	const char *saved_name1 = obs_hotkey_get_name(fixture.keys[1]);
	obs_hotkey_pair_set_descriptions(fixture.pair, desc0, desc1);
	assert_ptr_equal(obs_hotkey_get_name(fixture.keys[0]), saved_name0);
	assert_ptr_equal(obs_hotkey_get_name(fixture.keys[1]), saved_name1);
	assert_ptr_not_equal(obs_hotkey_get_description(fixture.keys[0]), desc0);
	assert_ptr_not_equal(obs_hotkey_get_description(fixture.keys[1]), desc1);
	desc0[0] = 'X';
	desc1[0] = 'Y';
	check_pair(&fixture, "new.0", "new.1", "New 0", "New 1");

	/* NULL clears just its own half; empty and raw bytes stay unnormalized.
	 * This nullable-string behavior is characterized, not endorsed. */
	obs_hotkey_pair_set_names(fixture.pair, NULL, "");
	check_pair(&fixture, NULL, "", "New 0", "New 1");
	obs_hotkey_pair_set_names(fixture.pair, "raw\377\200\t", NULL);
	check_pair(&fixture, "raw\377\200\t", NULL, "New 0", "New 1");
	obs_hotkey_pair_set_names(fixture.pair, NULL, NULL);
	check_pair(&fixture, NULL, NULL, "New 0", "New 1");
	obs_hotkey_pair_set_descriptions(fixture.pair, "", NULL);
	check_pair(&fixture, NULL, NULL, "", NULL);
	obs_hotkey_pair_set_descriptions(fixture.pair, NULL, "raw\377\200\t");
	check_pair(&fixture, NULL, NULL, NULL, "raw\377\200\t");
	obs_hotkey_pair_set_descriptions(fixture.pair, NULL, NULL);
	check_pair(&fixture, NULL, NULL, NULL, NULL);

	char shared[] = "shared\0ignored";
	obs_hotkey_pair_set_names(fixture.pair, shared, shared);
	obs_hotkey_pair_set_descriptions(fixture.pair, shared, shared);
	assert_ptr_not_equal(obs_hotkey_get_name(fixture.keys[0]), obs_hotkey_get_name(fixture.keys[1]));
	assert_ptr_not_equal(obs_hotkey_get_description(fixture.keys[0]), obs_hotkey_get_description(fixture.keys[1]));
	shared[0] = 'X';
	check_pair(&fixture, "shared", "shared", "shared", "shared");

	/* Removing either half does not remove the pair: the surviving half still
	 * changes, including half 1 when the first single setter finds no key. */
	for (size_t missing = 0; missing < 2; missing++) {
		obs_hotkey_pair_id pair = obs_hotkey_pair_register_frontend("half.0", "Half 0", "half.1", "Half 1",
									    no_pair, no_pair, NULL, NULL);
		assert_true(pair != OBS_INVALID_HOTKEY_PAIR_ID);
		obs_hotkey_t *halves[] = {named_key("half.0"), named_key("half.1")};
		obs_hotkey_id half_ids[] = {obs_hotkey_get_id(halves[0]), obs_hotkey_get_id(halves[1])};
		obs_hotkey_unregister(half_ids[missing]);
		fixture.signals = 0;
		obs_hotkey_pair_set_names(pair, "surviving.0", "surviving.1");
		obs_hotkey_pair_set_descriptions(pair, "Surviving 0", "Surviving 1");
		size_t survivor = 1 - missing;
		check_text(obs_hotkey_get_name(halves[survivor]), survivor == 0 ? "surviving.0" : "surviving.1");
		check_text(obs_hotkey_get_description(halves[survivor]), survivor == 0 ? "Surviving 0" : "Surviving 1");
		obs_hotkey_pair_set_names(pair, NULL, NULL);
		obs_hotkey_pair_set_descriptions(pair, NULL, NULL);
		assert_null(obs_hotkey_get_name(halves[survivor]));
		assert_null(obs_hotkey_get_description(halves[survivor]));
		assert_int_equal(fixture.signals, 0);
		obs_hotkey_unregister(half_ids[survivor]);
		fixture.signals = 0;
		obs_hotkey_pair_set_names(pair, "absent.0", "absent.1");
		obs_hotkey_pair_set_descriptions(pair, "Absent 0", "Absent 1");
		assert_int_equal(fixture.signals, 0);
		obs_hotkey_pair_unregister(pair);
		obs_hotkey_pair_set_names(pair, NULL, "deleted");
		obs_hotkey_pair_set_descriptions(pair, "deleted", NULL);
		check_pair(&fixture, "shared", "shared", "shared", "shared");
	}

	assert_int_equal(obs_hotkey_get_id(fixture.keys[0]), ids[0]);
	assert_int_equal(obs_hotkey_get_id(fixture.keys[1]), ids[1]);
	assert_int_equal(obs_hotkey_get_pair_partner_id(fixture.keys[0]), ids[1]);
	assert_int_equal(obs_hotkey_get_pair_partner_id(fixture.keys[1]), ids[0]);
	signal_handler_disconnect_global(signals, count_metadata_signals, &fixture);
	obs_hotkey_pair_unregister(fixture.pair);
	obs_hotkey_pair_set_names(fixture.pair, "deleted.0", "deleted.1");
	obs_hotkey_pair_set_descriptions(fixture.pair, NULL, NULL);
	obs_hotkey_pair_unregister(fixture.other);
	obs_hotkey_unregister(single_id);
	obs_shutdown();
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(pair_metadata_contract)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
