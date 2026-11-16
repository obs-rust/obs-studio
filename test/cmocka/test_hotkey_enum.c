/* Public-API characterization before extracting hotkey enumeration (#4419).
 * Context saves resolve live IDs, skipping IDs left by unregistration.
 * No private layouts or replacement implementations are used. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <cmocka.h>
#include <obs.h>
#include <string.h>

struct enumeration {
	size_t count;
	size_t limit;
	obs_hotkey_id ids[16];
	obs_key_combination_t combinations[16];
};

static bool collect_hotkey(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	struct enumeration *result = data;
	assert_true(result->count < 16);
	assert_int_equal(obs_hotkey_get_id(key), id);
	result->ids[result->count++] = id;
	return result->count < result->limit;
}

static bool collect_binding(void *data, size_t idx, obs_hotkey_binding_t *binding)
{
	struct enumeration *result = data;
	assert_int_equal(idx, result->count);
	assert_true(result->count < 16);
	assert_int_equal(obs_hotkey_get_id(obs_hotkey_binding_get_hotkey(binding)),
			 obs_hotkey_binding_get_hotkey_id(binding));
	result->ids[result->count] = obs_hotkey_binding_get_hotkey_id(binding);
	result->combinations[result->count++] = obs_hotkey_binding_get_key_combination(binding);
	return result->count < result->limit;
}

static void noop_hotkey(void *data, obs_hotkey_id id, obs_hotkey_t *key, bool pressed)
{
	(void)data;
	(void)id;
	(void)key;
	(void)pressed;
}

static const char *source_name(void *data)
{
	(void)data;
	return "hotkey enumeration";
}

static void *source_create(obs_data_t *settings, obs_source_t *source)
{
	(void)settings;
	return source;
}

static void source_destroy(void *data)
{
	(void)data;
}

static void no_task(void *data)
{
	(void)data;
}

static void enumeration_contract(void **state)
{
	(void)state;
	struct enumeration result = {.limit = 16};
	obs_enum_hotkeys(collect_hotkey, &result);
	assert_int_equal(result.count, 0);
	obs_enum_hotkey_bindings(collect_binding, &result);
	assert_int_equal(result.count, 0);

	assert_true(obs_startup("en-US", NULL, NULL));
	struct obs_source_info info = {
		.id = "hotkey_enum_input",
		.type = OBS_SOURCE_TYPE_INPUT,
		.get_name = source_name,
		.create = source_create,
		.destroy = source_destroy,
	};
	obs_register_source(&info);
	obs_source_t *source = obs_source_create(info.id, "enum", NULL, NULL);
	assert_non_null(source);
	obs_hotkey_id ids[3];
	const char *names[] = {"enum.first", "enum.removed", "enum.last"};
	for (size_t i = 0; i < 3; i++) {
		ids[i] = obs_hotkey_register_source(source, names[i], names[i], noop_hotkey, NULL);
		assert_true(ids[i] != OBS_INVALID_HOTKEY_ID);
	}
	obs_hotkey_unregister(ids[1]);

	result = (struct enumeration){.limit = 16};
	obs_enum_hotkeys(collect_hotkey, &result);
	assert_int_equal(result.count, 2);
	assert_int_equal(result.ids[0], ids[0]);
	assert_int_equal(result.ids[1], ids[2]);
	result = (struct enumeration){.limit = 1};
	obs_enum_hotkeys(collect_hotkey, &result);
	assert_int_equal(result.count, 1);
	assert_int_equal(result.ids[0], ids[0]);

	obs_key_combination_t combos[] = {{.modifiers = INTERACT_CONTROL_KEY, .key = OBS_KEY_A},
					  {.modifiers = INTERACT_SHIFT_KEY, .key = OBS_KEY_B}};
	obs_hotkey_load_bindings(ids[0], combos, 2);
	obs_hotkey_load_bindings(ids[2], combos + 1, 1);
	result = (struct enumeration){.limit = 16};
	obs_enum_hotkey_bindings(collect_binding, &result);
	assert_int_equal(result.count, 3);
	assert_int_equal(result.ids[0], ids[0]);
	assert_int_equal(result.ids[1], ids[0]);
	assert_int_equal(result.ids[2], ids[2]);
	for (size_t i = 0; i < 3; i++) {
		size_t expected = i == 0 ? 0 : 1;
		assert_int_equal(result.combinations[i].key, combos[expected].key);
		assert_int_equal(result.combinations[i].modifiers, combos[expected].modifiers);
	}
	result = (struct enumeration){.limit = 1};
	obs_enum_hotkey_bindings(collect_binding, &result);
	assert_int_equal(result.count, 1);

	/* Unregistration leaves a saved context ID; save must skip it and
	 * continue to the later live ID (characterized, not endorsed). */
	obs_data_t *saved = obs_hotkeys_save_source(source);
	assert_non_null(saved);
	assert_false(obs_data_has_user_value(saved, names[1]));
	for (size_t i = 0; i < 3; i += 2) {
		obs_data_array_t *bindings = obs_data_get_array(saved, names[i]);
		assert_non_null(bindings);
		assert_int_equal(obs_data_array_count(bindings), i == 0 ? 2 : 1);
		obs_data_array_release(bindings);
	}
	obs_data_release(saved);
	obs_source_release(source);
	obs_queue_task(OBS_TASK_DESTROY, no_task, NULL, true);
	result = (struct enumeration){.limit = 16};
	obs_enum_hotkeys(collect_hotkey, &result);
	assert_int_equal(result.count, 0);
	obs_enum_hotkey_bindings(collect_binding, &result);
	assert_int_equal(result.count, 0);
	obs_shutdown();
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(enumeration_contract)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
