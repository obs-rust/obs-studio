#pragma once

#include <cstdint>

#include <obs-config.h>

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
