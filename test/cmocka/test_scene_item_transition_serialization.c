/* Characterize the existing C scene-item transition serialization before its
 * extraction and Rust port. No graphics are needed for the empty/guard cases. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
#include <string.h>
#include <cmocka.h>
#include "util/threading.h"
#include "obs-internal.h"
#include "obs-scene.h"

static void empty_slots(void **state)
{
	(void)state;
	struct obs_scene_item item = {0};
	item.show_transition_duration = 123;
	item.hide_transition_duration = UINT32_MAX;
	for (unsigned show = 0; show < 2; show++) {
		obs_data_t *data = obs_sceneitem_transition_save(&item, show != 0);
		assert_non_null(data);
		assert_int_equal(obs_data_get_int(data, "duration"), show ? 123 : UINT32_MAX);
		assert_false(obs_data_has_user_value(data, "id"));
		assert_false(obs_data_has_user_value(data, "versioned_id"));
		assert_false(obs_data_has_user_value(data, "name"));
		assert_false(obs_data_has_user_value(data, "transition"));
		obs_data_release(data);
	}
}

static void load_guards_and_durations(void **state)
{
	(void)state;
	struct obs_scene_item item = {0};
	item.show_transition_duration = 11;
	item.hide_transition_duration = 22;
	unsigned char before[sizeof(item)];
	memcpy(before, &item, sizeof(item));
	obs_sceneitem_transition_load(NULL, NULL, false);
	obs_sceneitem_transition_load(&item, NULL, true);
	assert_memory_equal(&item, before, sizeof(item));
	obs_data_t *data = obs_data_create();
	obs_sceneitem_transition_load(NULL, data, true);
	const int64_t durations[] = {0, 1, -1, UINT32_MAX, (int64_t)UINT32_MAX + 2, INT64_MIN, INT64_MAX};
	for (unsigned show = 0; show < 2; show++) {
		for (size_t i = 0; i < sizeof(durations) / sizeof(durations[0]); i++) {
			item.show_transition_duration = 11;
			item.hide_transition_duration = 22;
			obs_data_set_int(data, "duration", durations[i]);
			obs_sceneitem_transition_load(&item, data, show != 0);
			/* Unsigned truncation is characterized, not endorsed. */
			assert_int_equal(show ? item.show_transition_duration : item.hide_transition_duration,
					 (uint32_t)durations[i]);
			assert_int_equal(show ? item.hide_transition_duration : item.show_transition_duration,
					 show ? 22 : 11);
			assert_null(item.show_transition);
			assert_null(item.hide_transition);
		}
	}
	obs_data_clear(data);
	obs_sceneitem_transition_load(&item, data, false);
	assert_int_equal(item.hide_transition_duration, 0);
	obs_data_release(data);
}

static void saved_identity_and_settings(void **state)
{
	(void)state;
	/* A borrowed source fixture exercises real source getters and obs_data
	 * reference ownership without starting core video or creating a source. */
	struct obs_source show_source = {0};
	struct obs_source hide_source = {0};
	show_source.info.id = "show_v2";
	show_source.info.unversioned_id = "show";
	show_source.context.name = "Show transition";
	show_source.context.settings = obs_data_create();
	hide_source.info.id = "hide_v3";
	hide_source.info.unversioned_id = "hide";
	hide_source.context.name = "Hide transition";
	hide_source.context.settings = obs_data_create();
	obs_data_set_string(show_source.context.settings, "marker", "show settings");
	obs_data_set_string(hide_source.context.settings, "marker", "hide settings");
	struct obs_scene_item item = {0};
	item.show_transition = &show_source;
	item.hide_transition = &hide_source;
	item.show_transition_duration = 125;
	item.hide_transition_duration = 750;
	for (unsigned show = 0; show < 2; show++) {
		obs_data_t *data = obs_sceneitem_transition_save(&item, show != 0);
		assert_string_equal(obs_data_get_string(data, "id"), show ? "show" : "hide");
		assert_string_equal(obs_data_get_string(data, "versioned_id"), show ? "show_v2" : "hide_v3");
		assert_string_equal(obs_data_get_string(data, "name"), show ? "Show transition" : "Hide transition");
		assert_int_equal(obs_data_get_int(data, "duration"), show ? 125 : 750);
		obs_data_t *settings = obs_data_get_obj(data, "transition");
		assert_ptr_equal(settings, show ? show_source.context.settings : hide_source.context.settings);
		assert_string_equal(obs_data_get_string(settings, "marker"), show ? "show settings" : "hide settings");
		obs_data_release(settings);
		obs_data_release(data);
	}
	/* Saving must not consume the sources' settings reference. */
	assert_string_equal(obs_data_get_string(show_source.context.settings, "marker"), "show settings");
	assert_string_equal(obs_data_get_string(hide_source.context.settings, "marker"), "hide settings");
	obs_data_release(show_source.context.settings);
	obs_data_release(hide_source.context.settings);
}

int main(void)
{
	const struct CMUnitTest tests[] = {cmocka_unit_test(empty_slots), cmocka_unit_test(load_guards_and_durations),
					   cmocka_unit_test(saved_identity_and_settings)};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
