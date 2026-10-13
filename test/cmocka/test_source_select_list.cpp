/* The existing sources the Add Source dialog lists for a source type
 * (frontend/utility/source-select-list.cpp, called by OBSBasicSourceSelect).
 * Before 32.2 the dialog sorted them by name; the new dialog listed them in
 * creation order. Uses real libobs sources of two dummy input types; no
 * video and no Qt GUI. obs_startup() needs a display on Linux (run under
 * xvfb-run); the cases skip when it cannot start. */

#include <obs.hpp>

#include "source-select-list.hpp"

#include <string>
#include <vector>

/* cmocka.h last: its fail() and inline macros break C++ standard headers. */
#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
extern "C" {
#include <cmocka.h>
}

static const char *dummy_name(void *)
{
	return "dummy";
}

static void *dummy_create(obs_data_t *, obs_source_t *source)
{
	return source;
}

static void dummy_destroy(void *) {}

static void register_type(const char *id)
{
	struct obs_source_info info = {};
	info.id = id;
	info.type = OBS_SOURCE_TYPE_INPUT;
	info.output_flags = OBS_SOURCE_VIDEO;
	info.get_name = dummy_name;
	info.create = dummy_create;
	info.destroy = dummy_destroy;
	obs_register_source(&info);
}

static bool obs_up;
static std::vector<obs_source_t *> created;
static std::vector<OBSWeakSource> weak_sources;

static obs_source_t *make(const char *type, const char *name)
{
	obs_source_t *source = obs_source_create(type, name, nullptr, nullptr);
	assert_non_null(source);
	created.push_back(source);
	OBSWeakSourceAutoRelease weak = obs_source_get_weak_source(source);
	weak_sources.emplace_back(weak);
	return source;
}

static std::string names(const std::vector<obs_weak_source_t *> &list)
{
	std::string out;
	for (obs_weak_source_t *weak : list) {
		OBSSourceAutoRelease source = obs_weak_source_get_source(weak);
		if (!out.empty()) {
			out += ",";
		}
		out += source ? obs_source_get_name(source) : "(gone)";
	}
	return out;
}

static int setup(void **)
{
	if (!obs_up) {
		return 0;
	}
	/* Created out of order, two types mixed, as in a real collection. */
	make("dummy_image", "Zeta");
	make("dummy_color", "red");
	make("dummy_image", "alpha");
	make("dummy_image", "Mike");
	make("dummy_color", "Blue");
	make("dummy_image", "beta");
	make("dummy_image", "Charlie");
	return 0;
}

static int teardown(void **)
{
	weak_sources.clear();
	for (obs_source_t *source : created) {
		obs_source_remove(source);
		obs_source_release(source);
	}
	created.clear();
	return 0;
}

/* A type's sources are listed by name, ignoring case. */
static void type_list_is_sorted_by_name(void **)
{
	if (!obs_up) {
		skip();
	}
	auto list = SourceSelectExistingList(weak_sources, "dummy_image", false);
	assert_string_equal(names(list).c_str(), "alpha,beta,Charlie,Mike,Zeta");

	list = SourceSelectExistingList(weak_sources, "dummy_color", false);
	assert_string_equal(names(list).c_str(), "Blue,red");
}

/* Guard: the Recently Added tab keeps creation order (the dialog shows it
 * newest first). */
static void recent_list_keeps_creation_order(void **)
{
	if (!obs_up) {
		skip();
	}
	auto list = SourceSelectExistingList(weak_sources, "_recent", true);
	assert_string_equal(names(list).c_str(), "Zeta,red,alpha,Mike,Blue,beta,Charlie");
}

/* Guard: removed sources are not listed. */
static void removed_source_is_not_listed(void **)
{
	if (!obs_up) {
		skip();
	}
	obs_source_remove(created[3]); /* Mike */
	auto list = SourceSelectExistingList(weak_sources, "dummy_image", false);
	assert_int_equal(list.size(), 4);
	assert_true(names(list).find("Mike") == std::string::npos);
}

int main(void)
{
	obs_up = obs_startup("en-US", nullptr, nullptr);
	if (obs_up) {
		register_type("dummy_image");
		register_type("dummy_color");
	}

	const struct CMUnitTest tests[] = {
		cmocka_unit_test_setup_teardown(type_list_is_sorted_by_name, setup, teardown),
		cmocka_unit_test_setup_teardown(recent_list_keeps_creation_order, setup, teardown),
		cmocka_unit_test_setup_teardown(removed_source_is_not_listed, setup, teardown),
	};
	int failed = cmocka_run_group_tests(tests, nullptr, nullptr);

	if (obs_up) {
		obs_shutdown();
	}
	return failed;
}
