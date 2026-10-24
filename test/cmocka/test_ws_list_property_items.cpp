/* obs-websocket GetInputPropertiesListPropertyItems must answer, not abort
 * OBS, when a list property holds an item with a NULL name or a NULL string
 * value (upstream obsproject/obs-studio#13905, obsproject/obs-websocket#1355).
 *
 * Utils::Obs::ArrayHelper::GetListPropertyItems() put the const char * from
 * obs_property_list_item_name() and obs_property_list_item_string() straight
 * into a json value. nlohmann::json builds a std::string from it, and a NULL
 * pointer throws std::logic_error ("basic_string: construction from null is
 * not valid") on the websocket thread, so OBS aborts. libobs keeps such items
 * as they were added: the macOS Screen Capture source puts a placeholder with
 * a NULL value at the top of its display and application lists, so asking for
 * those items took OBS down every time. Any plugin or script can do the same.
 *
 * This compiles the plugin's ArrayHelper (with the helpers it calls) against
 * real libobs. Properties need no libobs startup and no display. The frontend
 * calls the helpers reference are defined here; none of them runs. */

/* The C++ headers come before cmocka.h: its macros break libstdc++. */
#include <exception>
#include <string>
#include <vector>

#include <obs.h>
#include <util/platform.h>

#include "utils/Obs.h"

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
// cmocka.h 1.1 (Debian 13) and the Windows cmocka.h have no extern "C" guard.
extern "C" {
#include <cmocka.h>
}

extern "C" {
void obs_frontend_get_scenes(struct obs_frontend_source_list *) {}
void obs_frontend_get_transitions(struct obs_frontend_source_list *) {}
char **obs_frontend_get_scene_collections(void)
{
	return nullptr;
}
char **obs_frontend_get_profiles(void)
{
	return nullptr;
}
}
os_cpu_usage_info_t *GetCpuUsageInfo()
{
	return nullptr;
}
std::string Utils::Obs::StringHelper::GetCurrentRecordOutputPath()
{
	return "";
}

/* Runs the helper; an exception would escape the websocket thread and abort
 * OBS, so it fails the case instead. */
static std::vector<json> list_items(obs_property_t *property)
{
	try {
		return Utils::Obs::ArrayHelper::GetListPropertyItems(property);
	} catch (const std::exception &e) {
		fail_msg("GetListPropertyItems threw '%s' (OBS aborts on the websocket thread)", e.what());
	}
	return {};
}

static void assert_item(const json &item, const char *name, const json &value)
{
	if (!item["itemName"].is_string()) {
		fail_msg("itemName is %s, expected \"%s\"", item["itemName"].dump().c_str(), name);
	}
	assert_string_equal(item["itemName"].get<std::string>().c_str(), name);
	if (item["itemValue"] != value) {
		fail_msg("itemValue of \"%s\" is %s, expected %s", name, item["itemValue"].dump().c_str(),
			 value.dump().c_str());
	}
	assert_true(item["itemEnabled"].get<bool>());
}

/* Guard: ordinary string items keep their names and values. */
static void string_items_reported(void **)
{
	obs_properties_t *props = obs_properties_create();
	obs_property_t *list =
		obs_properties_add_list(props, "list", "List", OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_STRING);
	obs_property_list_add_string(list, "One", "one");
	obs_property_list_add_string(list, "Empty", "");

	std::vector<json> items = list_items(list);
	assert_int_equal(items.size(), 2);
	assert_item(items[0], "One", "one");
	assert_item(items[1], "Empty", "");
	obs_properties_destroy(props);
}

/* The macOS Screen Capture placeholder: a name and a NULL value. It is
 * reported with an empty value, which is what the properties dialog stores
 * when that entry is picked. */
static void null_string_value_reported_empty(void **)
{
	obs_properties_t *props = obs_properties_create();
	obs_property_t *list =
		obs_properties_add_list(props, "list", "List", OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_STRING);
	obs_property_list_add_string(list, " ", nullptr);
	obs_property_list_add_string(list, "One", "one");

	std::vector<json> items = list_items(list);
	assert_int_equal(items.size(), 2);
	assert_item(items[0], " ", "");
	assert_item(items[1], "One", "one");
	obs_properties_destroy(props);
}

/* An item added without a name, in a string list and in an int list. */
static void null_item_name_reported_empty(void **)
{
	obs_properties_t *props = obs_properties_create();
	obs_property_t *strings =
		obs_properties_add_list(props, "strings", "Strings", OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_STRING);
	obs_property_list_add_string(strings, nullptr, "unnamed");
	obs_property_t *ints =
		obs_properties_add_list(props, "ints", "Ints", OBS_COMBO_TYPE_LIST, OBS_COMBO_FORMAT_INT);
	obs_property_list_add_int(ints, nullptr, 7);
	obs_property_list_add_int(ints, "Eight", 8);

	std::vector<json> items = list_items(strings);
	assert_int_equal(items.size(), 1);
	assert_item(items[0], "", "unnamed");

	items = list_items(ints);
	assert_int_equal(items.size(), 2);
	assert_item(items[0], "", 7);
	assert_item(items[1], "Eight", 8);
	obs_properties_destroy(props);
}

int main(void)
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(string_items_reported),
		cmocka_unit_test(null_string_value_reported_empty),
		cmocka_unit_test(null_item_name_reported_empty),
	};
	return cmocka_run_group_tests(tests, NULL, NULL);
}
