#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdio.h>
#include <string.h>
#include <cmocka.h>
#include <obs.h>
#include <obs-hotkey.h>

/* Public API characterization: the paired latch exposes whether each callback
 * handled a press. No private fields or direct calls to the extracted bodies. */
struct key_lookup {
	const char *name;
	const char *description;
	obs_hotkey_id id;
	unsigned count;
};

static bool find_key(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	struct key_lookup *lookup = data;
	if (strcmp(obs_hotkey_get_name(key), lookup->name) == 0) {
		lookup->id = id;
		lookup->count++;
		if (lookup->description && *lookup->description)
			assert_string_equal(obs_hotkey_get_description(key), lookup->description);
		else if (lookup->description)
			/* Empty dstr templates become NULL descriptions: characterized, not endorsed. */
			assert_null(obs_hotkey_get_description(key));
	}
	return true;
}

static obs_hotkey_id lookup_key(obs_sceneitem_t *item, bool show, const char *description)
{
	char name[96];
	snprintf(name, sizeof(name), "libobs.%s_scene_item.%lld", show ? "show" : "hide",
		 (long long)obs_sceneitem_get_id(item));
	struct key_lookup lookup = {name, description, OBS_INVALID_HOTKEY_ID, 0};
	obs_enum_hotkeys(find_key, &lookup);
	assert_int_equal(lookup.count, 1);
	return lookup.id;
}

static void visible_signal(void *data, calldata_t *params)
{
	unsigned *count = data;
	assert_non_null(calldata_ptr(params, "item"));
	(*count)++;
}

static bool assert_key_absent(void *data, obs_hotkey_id id, obs_hotkey_t *key)
{
	(void)key;
	assert_true(id != *(obs_hotkey_id *)data);
	return true;
}

static void trigger_visibility(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create("hotkey-parent");
	obs_scene_t *child = obs_scene_create("hotkey-child");
	assert_non_null(scene);
	assert_non_null(child);
	obs_sceneitem_t *item = obs_scene_add(scene, obs_scene_get_source(child));
	assert_non_null(item);
	obs_sceneitem_addref(item);
	obs_hotkey_id show = lookup_key(item, true, "Show hotkey-child");
	obs_hotkey_id hide = lookup_key(item, false, "Hide hotkey-child");
	unsigned changes = 0;
	signal_handler_t *signals = obs_source_get_signal_handler(obs_scene_get_source(scene));
	signal_handler_connect(signals, "item_visible", visible_signal, &changes);
	assert_true(obs_sceneitem_visible(item));
	/* Releases and an already-visible show press do not handle the event and
	 * therefore must not latch show and block the following hide. */
	obs_hotkey_trigger_routed_callback(show, false);
	obs_hotkey_trigger_routed_callback(hide, false);
	obs_hotkey_trigger_routed_callback(show, true);
	assert_true(obs_sceneitem_visible(item));
	assert_int_equal(changes, 0);
	for (unsigned i = 0; i < 32; i++) {
		obs_hotkey_trigger_routed_callback(hide, true);
		assert_false(obs_sceneitem_visible(item));
		assert_int_equal(changes, 2 * i + 1);
		/* A handled hide press latches the pair until its release. */
		obs_hotkey_trigger_routed_callback(show, true);
		assert_false(obs_sceneitem_visible(item));
		obs_hotkey_trigger_routed_callback(hide, false);
		/* Already-hidden hide must not latch and block show. */
		obs_hotkey_trigger_routed_callback(hide, true);
		obs_hotkey_trigger_routed_callback(show, true);
		assert_true(obs_sceneitem_visible(item));
		assert_int_equal(changes, 2 * i + 2);
		obs_hotkey_trigger_routed_callback(hide, true);
		assert_true(obs_sceneitem_visible(item));
		obs_hotkey_trigger_routed_callback(show, false);
	}
	signal_handler_disconnect(signals, "item_visible", visible_signal, &changes);
	obs_sceneitem_remove(item);
	obs_sceneitem_release(item);
	/* Every acquired reference must have been released, including no-op
	 * paths: otherwise the item and its hotkey registrations remain alive. */
	obs_enum_hotkeys(assert_key_absent, &show);
	obs_enum_hotkeys(assert_key_absent, &hide);
	/* Destroyed item registrations are inert through the public trigger API. */
	obs_hotkey_trigger_routed_callback(show, true);
	obs_hotkey_trigger_routed_callback(hide, true);
	obs_source_remove(obs_scene_get_source(child));
	obs_scene_release(child);
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	while (obs_wait_for_destroy_queue())
		;
}

static void renamed_descriptions(void **state)
{
	(void)state;
	obs_scene_t *scene = obs_scene_create("rename-parent");
	obs_scene_t *child = obs_scene_create("original");
	assert_non_null(scene);
	assert_non_null(child);
	obs_source_t *source = obs_scene_get_source(child);
	obs_sceneitem_t *first = obs_scene_add(scene, source);
	obs_sceneitem_t *second = obs_scene_add(scene, source);
	assert_non_null(first);
	assert_non_null(second);
	obs_hotkey_id show = lookup_key(first, true, "Show original");
	obs_hotkey_id hide = lookup_key(first, false, "Hide original");
	/* Renaming uses the current templates, replaces every placeholder, and
	 * updates every item that references this source without changing IDs. */
	obs_hotkeys_set_sceneitem_hotkeys_translations("[%1] show %1", "hide <%1>");
	lookup_key(first, true, "Show original");
	obs_source_set_name(source, "renamed");
	assert_int_equal(lookup_key(first, true, "[renamed] show renamed"), show);
	assert_int_equal(lookup_key(first, false, "hide <renamed>"), hide);
	lookup_key(second, true, "[renamed] show renamed");
	lookup_key(second, false, "hide <renamed>");
	obs_hotkeys_set_sceneitem_hotkeys_translations("literal", "");
	obs_source_set_name(source, "third");
	lookup_key(first, true, "literal");
	lookup_key(first, false, "");
	lookup_key(second, true, "literal");
	lookup_key(second, false, "");
	obs_source_remove(obs_scene_get_source(scene));
	obs_scene_release(scene);
	obs_source_remove(obs_scene_get_source(child));
	obs_scene_release(child);
	while (obs_wait_for_destroy_queue())
		;
}

static int setup(void **state)
{
	(void)state;
	assert_true(obs_startup("en-US", NULL, NULL));
	obs_hotkeys_set_sceneitem_hotkeys_translations("Show %1", "Hide %1");
	obs_hotkey_enable_callback_rerouting(true);
	return 0;
}

static int teardown(void **state)
{
	(void)state;
	obs_shutdown();
	return 0;
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(trigger_visibility),
					   cmocka_unit_test(renamed_descriptions)};
	return cmocka_run_group_tests(tests, setup, teardown);
}
