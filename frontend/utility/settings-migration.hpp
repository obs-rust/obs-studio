#pragma once

#include <cstdint>
#include <filesystem>
#include <system_error>
#include <utility>

#include <obs-config.h>
#include <util/base.h>

/* The pre-31 settings migration step of OBSApp::InitGlobalConfig, kept free of
 * Qt so it can be unit tested (obs-rust/obs-studio#815). Before OBS 31.0, the
 * user settings were in global.ini; MigrateGlobalSettings() copies them to
 * user.ini. */

struct LegacySettingsState {
	/* [General] LastVersion in global.ini, 0 when missing. OBS writes it only
	 * on a clean exit. */
	uint32_t lastVersion;
	/* [General] Pre31Migrated in global.ini. */
	bool alreadyMigrated;
	/* [General] FirstRun is set in global.ini. OBS before 31.0 writes it there
	 * on its first start; OBS 31.0 and later write it to user.ini. */
	bool globalHasFirstRun;
	/* user.ini exists. */
	bool userConfigExists;
};

inline bool ShouldMigratePre31Settings(const LegacySettingsState &state)
{
	if (state.alreadyMigrated) {
		return false;
	}

	if (state.lastVersion) {
		return state.lastVersion < MAKE_SEMANTIC_VERSION(31, 0, 0);
	}

	/* No LastVersion: OBS never exited cleanly. A FirstRun in global.ini
	 * still shows that OBS before 31.0 wrote it. Without user.ini, migrate;
	 * with user.ini, MigrateGlobalSettings() would fail with an error box. */
	return state.globalHasFirstRun && !state.userConfigExists;
}

/* The settings path checks of OBSApp::InitGlobalConfig: whether user.ini
 * exists, and each [Locations] path of global.ini (Configuration,
 * SceneCollections, Profiles, PluginManagerSettings), used when it exists,
 * else its default.
 *
 * A path that can't be checked (permission denied, an I/O error, a name too
 * long) counts as missing and is logged. The throwing exists() aborted OBS
 * at every start instead (obs-rust/obs-studio#5569, upstream
 * obsproject/obs-studio#14253). */
inline bool SettingsPathExists(const std::filesystem::path &path)
{
	std::error_code error;
	const bool exists = std::filesystem::exists(path, error);

	if (error) {
		blog(LOG_WARNING, "Unable to check settings path '%s': %s", path.u8string().c_str(),
		     error.message().c_str());
		return false;
	}

	return exists;
}

inline std::filesystem::path ChooseSettingsLocation(std::filesystem::path saved, std::filesystem::path fallback)
{
	return SettingsPathExists(saved) ? std::move(saved) : std::move(fallback);
}
