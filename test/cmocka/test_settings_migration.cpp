// The C++ headers go first: cmocka.h defines macros (fail, ...) that clash
// with libstdc++.
#include <cstdint>
#include <filesystem>
#include <string>

#include <stdarg.h>
#include <stddef.h>
#include <setjmp.h>
#include <stdint.h>
// The Windows cmocka.h has no extern "C" guard of its own.
extern "C" {
#include <cmocka.h>
}

#include <util/c99defs.h>

#include "settings-migration.hpp"

/* obs-rust/obs-studio#815: a global.ini written by OBS before 31.0 that has
 * no LastVersion (OBS never exited cleanly, for example because it ran as a
 * service and was always killed) was treated as a fresh install. The user
 * settings were not migrated to user.ini, and the first start showed the
 * auto-configuration wizard, which disables all outputs while it is open. */

static const uint32_t v30_2_3 = MAKE_SEMANTIC_VERSION(30, 2, 3);
static const uint32_t v32 = MAKE_SEMANTIC_VERSION(32, 0, 0);

static void missing_last_version_with_legacy_settings_migrates(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(ShouldMigratePre31Settings({0, false, true, false}));
}

static void fresh_install_does_not_migrate(void **state)
{
	UNUSED_PARAMETER(state);
	assert_false(ShouldMigratePre31Settings({0, false, false, false}));
}

static void missing_last_version_with_user_config_does_not_migrate(void **state)
{
	UNUSED_PARAMETER(state);
	assert_false(ShouldMigratePre31Settings({0, false, true, true}));
}

static void pre31_version_migrates(void **state)
{
	UNUSED_PARAMETER(state);
	assert_true(ShouldMigratePre31Settings({v30_2_3, false, true, false}));
}

static void already_migrated_does_not_migrate(void **state)
{
	UNUSED_PARAMETER(state);
	assert_false(ShouldMigratePre31Settings({v30_2_3, true, true, false}));
}

static void version_31_or_later_does_not_migrate(void **state)
{
	UNUSED_PARAMETER(state);
	assert_false(ShouldMigratePre31Settings({v32, false, true, true}));
}

/* obs-rust/obs-studio#5569 (upstream obsproject/obs-studio#14253): a saved
 * settings location whose check fails with an error other than "not found"
 * (permission denied, an I/O error on a mounted drive, ...) made the throwing
 * std::filesystem::exists() abort OBS at every start. A path component longer
 * than any file name gives ENAMETOOLONG, also as root. */

static std::filesystem::path unreadable_path()
{
	return std::filesystem::temp_directory_path() / std::string(300, 'x') / "obs-studio";
}

static std::filesystem::path missing_path()
{
	return std::filesystem::temp_directory_path() / "obs-rust-5569-missing-location";
}

static void existing_saved_location_is_used(void **state)
{
	UNUSED_PARAMETER(state);
	const std::filesystem::path saved = std::filesystem::temp_directory_path();
	assert_true(ChooseSettingsLocation(saved, missing_path()) == saved);
}

static void missing_saved_location_falls_back(void **state)
{
	UNUSED_PARAMETER(state);
	const std::filesystem::path fallback = std::filesystem::temp_directory_path();
	assert_true(ChooseSettingsLocation(missing_path(), fallback) == fallback);
}

static void unreadable_saved_location_falls_back(void **state)
{
	UNUSED_PARAMETER(state);
	const std::filesystem::path fallback = std::filesystem::temp_directory_path();
	assert_true(ChooseSettingsLocation(unreadable_path(), fallback) == fallback);
}

static void unreadable_user_config_counts_as_missing(void **state)
{
	UNUSED_PARAMETER(state);
	assert_false(SettingsPathExists(unreadable_path() / "user.ini"));
}

int main()
{
	const struct CMUnitTest tests[] = {
		cmocka_unit_test(missing_last_version_with_legacy_settings_migrates),
		cmocka_unit_test(fresh_install_does_not_migrate),
		cmocka_unit_test(missing_last_version_with_user_config_does_not_migrate),
		cmocka_unit_test(pre31_version_migrates),
		cmocka_unit_test(already_migrated_does_not_migrate),
		cmocka_unit_test(version_31_or_later_does_not_migrate),
		cmocka_unit_test(existing_saved_location_is_used),
		cmocka_unit_test(missing_saved_location_falls_back),
		cmocka_unit_test(unreadable_saved_location_falls_back),
		cmocka_unit_test(unreadable_user_config_counts_as_missing),
	};

	return cmocka_run_group_tests(tests, NULL, NULL);
}
