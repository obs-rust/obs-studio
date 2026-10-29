/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#include "frontend/build/frontend-app-obsapp.pch.hpp"

using namespace std;

string currentLogFile;
string lastLogFile;
string lastCrashLogFile;

extern bool portable_mode;
extern bool safe_mode;
extern bool multi;
extern bool disable_3p_plugins;
extern bool disable_shutdown_check;
extern bool opt_disable_updater;
extern bool opt_disable_missing_files_check;
extern string opt_starting_collection;
extern string opt_starting_profile;

constexpr double kDefaultSnapDistance = 5.0;

namespace {

typedef struct UncleanLaunchAction {
	bool useSafeMode = false;
	bool sendCrashReport = false;
} UncleanLaunchAction;

enum class PluginFailureAction { Continue, OpenPluginManager };

PluginFailureAction handlePluginFailure()
{
	QMessageBox pluginWarning;

	pluginWarning.setIcon(QMessageBox::Warning);

	pluginWarning.setWindowTitle(QTStr("PluginFailure.Dialog.Title"));
	pluginWarning.setText(QTStr("PluginFailure.Labels.Text"));

	QPushButton *continueButton =
		pluginWarning.addButton(QTStr("PluginFailure.Dialog.Continue"), QMessageBox::RejectRole);
	QPushButton *handleButton =
		pluginWarning.addButton(QTStr("PluginFailure.Dialog.Open"), QMessageBox::AcceptRole);

	pluginWarning.setDefaultButton(continueButton);

	pluginWarning.exec();

	bool openPluginManager = pluginWarning.clickedButton() == handleButton;

	if (openPluginManager) {
		return PluginFailureAction::OpenPluginManager;
	} else {
		return PluginFailureAction::Continue;
	}
}

UncleanLaunchAction handleUncleanShutdown(bool enableCrashUpload)
{
	UncleanLaunchAction launchAction;

	blog(LOG_WARNING, "Crash or unclean shutdown detected");

	QMessageBox crashWarning;

	crashWarning.setIcon(QMessageBox::Warning);
	crashWarning.setOption(QMessageBox::Option::DontUseNativeDialog);
	crashWarning.setWindowTitle(QTStr("CrashHandling.Dialog.Title"));
	crashWarning.setText(QTStr("CrashHandling.Labels.Text"));

	if (enableCrashUpload) {
		crashWarning.setInformativeText(QTStr("CrashHandling.Labels.LocalNotice"));

		QCheckBox *sendCrashReportCheckbox = new QCheckBox(QTStr("CrashHandling.Checkbox.OpenFolder"));
		crashWarning.setCheckBox(sendCrashReportCheckbox);
	}

	QPushButton *launchSafeButton =
		crashWarning.addButton(QTStr("CrashHandling.Buttons.LaunchSafe"), QMessageBox::AcceptRole);
	QPushButton *launchNormalButton =
		crashWarning.addButton(QTStr("CrashHandling.Buttons.LaunchNormal"), QMessageBox::RejectRole);

	crashWarning.setDefaultButton(launchNormalButton);

	crashWarning.exec();

	bool useSafeMode = crashWarning.clickedButton() == launchSafeButton;

	if (useSafeMode) {
		launchAction.useSafeMode = true;

		blog(LOG_INFO, "[Safe Mode] Safe mode launch selected, loading third-party plugins is disabled");
	} else {
		blog(LOG_WARNING, "[Safe Mode] Normal launch selected, loading third-party plugins is enabled");
	}

	bool sendCrashReport = (enableCrashUpload) ? crashWarning.checkBox()->isChecked() : false;

	if (sendCrashReport) {
		launchAction.sendCrashReport = true;

		blog(LOG_INFO, "User selected to send crash report");
	}

	return launchAction;
}

QAccessibleInterface *alignmentSelectorFactory(const QString &classname, QObject *object)
{
	if (classname == QLatin1String("AlignmentSelector")) {
		if (auto *w = qobject_cast<AlignmentSelector *>(object)) {
			return new AccessibleAlignmentSelector(w);
		}
	}
	return nullptr;
}
} // namespace

QObject *CreateShortcutFilter(QObject *parent)
{
	auto *filter = new OBSEventFilter([](QObject *obj, QEvent *event) {
		auto mouse_event = [](QMouseEvent &event) {
			if (!App()->HotkeysEnabledInFocus() && event.button() != Qt::LeftButton) {
				return true;
			}

			obs_key_combination_t hotkey = {0, OBS_KEY_NONE};
			bool pressed = event.type() == QEvent::MouseButtonPress;

			switch (event.button()) {
			case Qt::NoButton:
			case Qt::LeftButton:
			case Qt::RightButton:
			case Qt::AllButtons:
			case Qt::MouseButtonMask:
				return false;

			case Qt::MiddleButton:
				hotkey.key = OBS_KEY_MOUSE3;
				break;

#define MAP_BUTTON(i, j)                       \
	case Qt::ExtraButton##i:               \
		hotkey.key = OBS_KEY_MOUSE##j; \
		break;
				MAP_BUTTON(1, 4);
				MAP_BUTTON(2, 5);
				MAP_BUTTON(3, 6);
				MAP_BUTTON(4, 7);
				MAP_BUTTON(5, 8);
				MAP_BUTTON(6, 9);
				MAP_BUTTON(7, 10);
				MAP_BUTTON(8, 11);
				MAP_BUTTON(9, 12);
				MAP_BUTTON(10, 13);
				MAP_BUTTON(11, 14);
				MAP_BUTTON(12, 15);
				MAP_BUTTON(13, 16);
				MAP_BUTTON(14, 17);
				MAP_BUTTON(15, 18);
				MAP_BUTTON(16, 19);
				MAP_BUTTON(17, 20);
				MAP_BUTTON(18, 21);
				MAP_BUTTON(19, 22);
				MAP_BUTTON(20, 23);
				MAP_BUTTON(21, 24);
				MAP_BUTTON(22, 25);
				MAP_BUTTON(23, 26);
				MAP_BUTTON(24, 27);
#undef MAP_BUTTON
			}

			hotkey.modifiers = TranslateQtKeyboardEventModifiers(event.modifiers());

			obs_hotkey_inject_event(hotkey, pressed);
			return true;
		};

		auto key_event = [&](QKeyEvent *event) {
			int key = event->key();
			bool enabledInFocus = App()->HotkeysEnabledInFocus();

			if (key != Qt::Key_Enter && key != Qt::Key_Escape && key != Qt::Key_Return && !enabledInFocus)
				return true;

			QDialog *dialog = qobject_cast<QDialog *>(obj);

			obs_key_combination_t hotkey = {0, OBS_KEY_NONE};
			bool pressed = event->type() == QEvent::KeyPress;

			switch (key) {
			case Qt::Key_Shift:
			case Qt::Key_Control:
			case Qt::Key_Alt:
			case Qt::Key_Meta:
				break;

			case Qt::Key_Enter:
			case Qt::Key_Escape:
			case Qt::Key_Return:
				if (dialog && pressed)
					return false;
				if (!enabledInFocus)
					return true;
				/* Falls through. */
			default:
				if (GetPlatformHotkeyOverride(key, hotkey.key, pressed)) {
					break;
				}
				hotkey.key = obs_key_from_virtual_key(event->nativeVirtualKey());
			}

			if (event->isAutoRepeat())
				return true;

			hotkey.modifiers = TranslateQtKeyboardEventModifiers(event->modifiers());

			obs_hotkey_inject_event(hotkey, pressed);
			return true;
		};

		switch (event->type()) {
		case QEvent::MouseButtonPress:
		case QEvent::MouseButtonRelease:
			return mouse_event(*static_cast<QMouseEvent *>(event));

		/*case QEvent::MouseButtonDblClick:
		case QEvent::Wheel:*/
		case QEvent::KeyPress:
		case QEvent::KeyRelease:
			return key_event(static_cast<QKeyEvent *>(event));

		default:
			return false;
		}
	});
	filter->setParent(parent);
	return filter;
}

string CurrentDateTimeString()
{
	time_t now = time(0);
	struct tm tstruct;
	char buf[80];
	PlatformLocalTime(&now, &tstruct);
	strftime(buf, sizeof(buf), "%Y-%m-%d, %X", &tstruct);
	return buf;
}

#define DEFAULT_LANG "en-US"

bool OBSApp::InitGlobalConfigDefaults()
{
	config_set_default_uint(appConfig, "General", "MaxLogs", 10);
	config_set_default_int(appConfig, "General", "InfoIncrement", -1);
	config_set_default_string(appConfig, "General", "ProcessPriority", "Normal");
	config_set_default_bool(appConfig, "General", "EnableAutoUpdates", true);

	InitPlatformConfigDefaults(appConfig);

	return true;
}

bool OBSApp::InitGlobalLocationDefaults()
{
	char path[512];

	int len = GetAppConfigPath(path, sizeof(path), nullptr);
	if (len <= 0) {
		OBSErrorBox(NULL, "Unable to get global configuration path.");
		return false;
	}

	config_set_default_string(appConfig, "Locations", "Configuration", path);
	config_set_default_string(appConfig, "Locations", "SceneCollections", path);
	config_set_default_string(appConfig, "Locations", "Profiles", path);
	config_set_default_string(appConfig, "Locations", "PluginManagerSettings", path);

	return true;
}

void OBSApp::InitUserConfigDefaults()
{
	config_set_default_bool(userConfig, "General", "ConfirmOnExit", true);

	config_set_default_string(userConfig, "General", "HotkeyFocusType", "NeverDisableHotkeys");

	config_set_default_bool(userConfig, "BasicWindow", "PreviewEnabled", true);
	config_set_default_bool(userConfig, "BasicWindow", "PreviewProgramMode", false);
	config_set_default_bool(userConfig, "BasicWindow", "SceneDuplicationMode", true);
	config_set_default_bool(userConfig, "BasicWindow", "SwapScenesMode", true);
	config_set_default_bool(userConfig, "BasicWindow", "SnappingEnabled", true);
	config_set_default_bool(userConfig, "BasicWindow", "ScreenSnapping", true);
	config_set_default_bool(userConfig, "BasicWindow", "SourceSnapping", true);
	config_set_default_bool(userConfig, "BasicWindow", "CenterSnapping", false);
	config_set_default_double(userConfig, "BasicWindow", "SnapDistance", kDefaultSnapDistance);
	config_set_default_bool(userConfig, "BasicWindow", "SpacingHelpersEnabled", true);
	config_set_default_bool(userConfig, "BasicWindow", "RecordWhenStreaming", false);
	config_set_default_bool(userConfig, "BasicWindow", "KeepRecordingWhenStreamStops", false);
	config_set_default_bool(userConfig, "BasicWindow", "SysTrayEnabled", true);
	config_set_default_bool(userConfig, "BasicWindow", "SysTrayWhenStarted", false);
	config_set_default_bool(userConfig, "BasicWindow", "SaveProjectors", false);
	config_set_default_bool(userConfig, "BasicWindow", "ShowTransitions", true);
	config_set_default_bool(userConfig, "BasicWindow", "ShowListboxToolbars", true);
	config_set_default_bool(userConfig, "BasicWindow", "ShowStatusBar", true);
	config_set_default_bool(userConfig, "BasicWindow", "ShowSourceIcons", true);
	config_set_default_bool(userConfig, "BasicWindow", "ShowContextToolbars", true);
	config_set_default_bool(userConfig, "BasicWindow", "StudioModeLabels", true);
	config_set_default_bool(userConfig, "BasicWindow", "SideDocks", true);

	config_set_default_bool(userConfig, "BasicWindow", "VerticalVolumeControl", true);

	config_set_default_bool(userConfig, "BasicWindow", "MultiviewMouseSwitch", true);

	config_set_default_bool(userConfig, "BasicWindow", "MultiviewDrawNames", true);

	config_set_default_bool(userConfig, "BasicWindow", "MultiviewDrawAreas", true);

	config_set_default_bool(userConfig, "BasicWindow", "MediaControlsCountdownTimer", true);

	config_set_default_bool(App()->GetUserConfig(), "BasicWindow", "MixerShowInactive", false);
	config_set_default_bool(App()->GetUserConfig(), "BasicWindow", "MixerKeepInactiveLast", false);
	config_set_default_bool(App()->GetUserConfig(), "BasicWindow", "MixerShowHidden", false);
	config_set_default_bool(App()->GetUserConfig(), "BasicWindow", "MixerKeepHiddenLast", false);

	config_set_default_int(userConfig, "Appearance", "FontScale", 10);
	config_set_default_int(userConfig, "Appearance", "Density", 1);
}

static bool do_mkdir(const char *path)
{
	if (os_mkdirs(path) == MKDIR_ERROR) {
		OBSErrorBox(NULL, "Failed to create directory %s", path);
		return false;
	}

	return true;
}

static bool MakeUserDirs()
{
	char path[512];

	if (GetAppConfigPath(path, sizeof(path), "obs-studio/basic") <= 0) {
		return false;
	}
	if (!do_mkdir(path)) {
		return false;
	}

	if (GetAppConfigPath(path, sizeof(path), "obs-studio/logs") <= 0) {
		return false;
	}
	if (!do_mkdir(path)) {
		return false;
	}

	if (GetAppConfigPath(path, sizeof(path), "obs-studio/profiler_data") <= 0) {
		return false;
	}
	if (!do_mkdir(path)) {
		return false;
	}

	for (const std::string &dir : GetPlatformExtraConfigDirs()) {
		if (GetAppConfigPath(path, sizeof(path), dir.c_str()) <= 0) {
			return false;
		}
		if (!do_mkdir(path)) {
			return false;
		}
	}

	if (GetAppConfigPath(path, sizeof(path), "obs-studio/updates") <= 0) {
		return false;
	}
	if (!do_mkdir(path)) {
		return false;
	}

	if (GetAppConfigPath(path, sizeof(path), "obs-studio/plugin_config") <= 0) {
		return false;
	}
	if (!do_mkdir(path)) {
		return false;
	}

	return true;
}

constexpr std::string_view OBSProfileSubDirectory = OBS_CONFIG_DIRECTORY "/basic/profiles";
constexpr std::string_view OBSScenesSubDirectory = OBS_CONFIG_DIRECTORY "/basic/scenes";
constexpr std::string_view OBSPluginManagerSubDirectory = OBS_CONFIG_DIRECTORY "/plugin_manager";

static bool MakeUserProfileDirs()
{
	const std::filesystem::path userProfilePath =
		App()->userProfilesLocation / std::filesystem::u8path(OBSProfileSubDirectory);
	const std::filesystem::path userScenesPath =
		App()->userScenesLocation / std::filesystem::u8path(OBSScenesSubDirectory);
	const std::filesystem::path userPluginManagerPath =
		App()->userPluginManagerSettingsLocation / std::filesystem::u8path(OBSPluginManagerSubDirectory);

	if (!std::filesystem::exists(userProfilePath)) {
		try {
			std::filesystem::create_directories(userProfilePath);
		} catch (const std::filesystem::filesystem_error &error) {
			blog(LOG_ERROR, "Failed to create user profile directory '%s'\n%s",
			     userProfilePath.u8string().c_str(), error.what());
			return false;
		}
	}

	if (!std::filesystem::exists(userScenesPath)) {
		try {
			std::filesystem::create_directories(userScenesPath);
		} catch (const std::filesystem::filesystem_error &error) {
			blog(LOG_ERROR, "Failed to create user scene collection directory '%s'\n%s",
			     userScenesPath.u8string().c_str(), error.what());
			return false;
		}
	}

	if (!std::filesystem::exists(userPluginManagerPath)) {
		try {
			std::filesystem::create_directories(userPluginManagerPath);
		} catch (const std::filesystem::filesystem_error &error) {
			blog(LOG_ERROR, "Failed to create user plugin manager directory '%s'\n%s",
			     userPluginManagerPath.u8string().c_str(), error.what());
			return false;
		}
	}

	return true;
}

bool OBSApp::UpdatePre22MultiviewLayout(const char *layout)
{
	if (!layout) {
		return false;
	}

	if (astrcmpi(layout, "horizontaltop") == 0) {
		config_set_int(userConfig, "BasicWindow", "MultiviewLayout",
			       static_cast<int>(MultiviewLayout::HORIZONTAL_TOP_8_SCENES));
		return true;
	}

	if (astrcmpi(layout, "horizontalbottom") == 0) {
		config_set_int(userConfig, "BasicWindow", "MultiviewLayout",
			       static_cast<int>(MultiviewLayout::HORIZONTAL_BOTTOM_8_SCENES));
		return true;
	}

	if (astrcmpi(layout, "verticalleft") == 0) {
		config_set_int(userConfig, "BasicWindow", "MultiviewLayout",
			       static_cast<int>(MultiviewLayout::VERTICAL_LEFT_8_SCENES));
		return true;
	}

	if (astrcmpi(layout, "verticalright") == 0) {
		config_set_int(userConfig, "BasicWindow", "MultiviewLayout",
			       static_cast<int>(MultiviewLayout::VERTICAL_RIGHT_8_SCENES));
		return true;
	}

	return false;
}

bool OBSApp::MigrateRustConfiguration()
{
#ifdef OBS_RUST_VERSION
	namespace fs = std::filesystem;
	try {
		BPtr<char> configRoot = GetAppConfigPathPtr(nullptr);
		if (!configRoot) {
			throw std::runtime_error("Unable to locate the configuration root");
		}
		const auto base = ConfigBaseDirectory(configRoot.Get());
		const auto source = base / "obs-studio";
		const auto destination = base / OBS_CONFIG_DIRECTORY;
		if (fs::exists(destination) || !fs::exists(source)) {
			return true;
		}
		fs::create_directories(base);
		QLockFile lock(QString::fromStdString((base / "obs-studio-rust.migration.lock").u8string()));
		if (!lock.tryLock(0)) {
			throw std::runtime_error("Another Rust frontend is migrating its configuration");
		}
		ConfigFile upstream;
		const auto global = source / "global.ini";
		if (fs::exists(global) &&
		    upstream.Open(global.u8string().c_str(), CONFIG_OPEN_EXISTING) != CONFIG_SUCCESS) {
			throw std::runtime_error("Unable to read the original global settings");
		}
		auto location = [&](const char *key) {
			if (!IsPortableMode() && fs::exists(global)) {
				const char *value = config_get_string(upstream, "Locations", key);
				if (value && *value && fs::is_directory(fs::u8path(value))) {
					return fs::u8path(value);
				}
			}
			return base;
		};
		std::vector<std::pair<const char *, fs::path>> locations;
		std::vector<fs::path> roots;
		for (const char *key : {"Configuration", "Profiles", "SceneCollections", "PluginManagerSettings"}) {
			auto selected = location(key);
			roots.push_back(selected);
			locations.emplace_back(key, std::move(selected));
		}
		MigrateAppConfigLocations(base, roots, [&](const fs::path &copy) {
			ConfigFile isolated;
			if (isolated.Open((copy / "global.ini").u8string().c_str(), CONFIG_OPEN_ALWAYS) !=
			    CONFIG_SUCCESS) {
				throw std::runtime_error("Unable to open the copied global settings");
			}
			for (const auto &[key, root] : locations) {
				config_set_string(isolated, "Locations", key, root.u8string().c_str());
			}
			config_set_string(isolated, "General", "RustConfigImportedFrom", source.u8string().c_str());
			if (isolated.SaveSafe("tmp") != CONFIG_SUCCESS) {
				throw std::runtime_error("Unable to save the independent settings locations");
			}
		});
	} catch (const std::exception &error) {
		OBSErrorBox(nullptr, "Rust configuration migration failed: %s. Original settings were preserved.",
			    error.what());
		return false;
	}
#endif
	return true;
}

bool OBSApp::InitGlobalConfig()
{
	char path[512];

	int len = GetAppConfigPath(path, sizeof(path), OBS_CONFIG_DIRECTORY "/global.ini");
	if (len <= 0) {
		return false;
	}

	int errorcode = appConfig.Open(path, CONFIG_OPEN_ALWAYS);
	if (errorcode != CONFIG_SUCCESS) {
		OBSErrorBox(NULL, "Failed to open global.ini: %d", errorcode);
		return false;
	}

	uint32_t lastVersion = config_get_int(appConfig, "General", "LastVersion");

	LegacySettingsState legacySettings{};
	legacySettings.lastVersion = lastVersion;
	legacySettings.alreadyMigrated = config_get_bool(appConfig, "General", "Pre31Migrated");
	legacySettings.globalHasFirstRun = config_has_user_value(appConfig, "General", "FirstRun");

	char configRoot[512];
	if (GetAppConfigPath(configRoot, sizeof(configRoot), nullptr) > 0) {
		std::string userConfigFile{configRoot};
		userConfigFile.append("/" OBS_CONFIG_DIRECTORY "/user.ini");
		legacySettings.userConfigExists = std::filesystem::exists(std::filesystem::u8path(userConfigFile));
	}

	if (ShouldMigratePre31Settings(legacySettings)) {
		bool migrated = MigrateGlobalSettings();

		config_set_bool(appConfig, "General", "Pre31Migrated", migrated);
		config_save_safe(appConfig, "tmp", nullptr);
	}

	InitGlobalConfigDefaults();
	InitGlobalLocationDefaults();

	std::filesystem::path defaultUserConfigLocation =
		std::filesystem::u8path(config_get_default_string(appConfig, "Locations", "Configuration"));
	std::filesystem::path defaultUserScenesLocation =
		std::filesystem::u8path(config_get_default_string(appConfig, "Locations", "SceneCollections"));
	std::filesystem::path defaultUserProfilesLocation =
		std::filesystem::u8path(config_get_default_string(appConfig, "Locations", "Profiles"));
	std::filesystem::path defaultPluginManagerLocation =
		std::filesystem::u8path(config_get_default_string(appConfig, "Locations", "PluginManagerSettings"));

	if (IsPortableMode()) {
		userConfigLocation = std::move(defaultUserConfigLocation);
		userScenesLocation = std::move(defaultUserScenesLocation);
		userProfilesLocation = std::move(defaultUserProfilesLocation);
		userPluginManagerSettingsLocation = std::move(defaultPluginManagerLocation);
	} else {
		std::filesystem::path currentUserConfigLocation =
			std::filesystem::u8path(config_get_string(appConfig, "Locations", "Configuration"));
		std::filesystem::path currentUserScenesLocation =
			std::filesystem::u8path(config_get_string(appConfig, "Locations", "SceneCollections"));
		std::filesystem::path currentUserProfilesLocation =
			std::filesystem::u8path(config_get_string(appConfig, "Locations", "Profiles"));
		std::filesystem::path currentUserPluginManagerLocation =
			std::filesystem::u8path(config_get_string(appConfig, "Locations", "PluginManagerSettings"));

		userConfigLocation = (std::filesystem::exists(currentUserConfigLocation))
					     ? std::move(currentUserConfigLocation)
					     : std::move(defaultUserConfigLocation);
		userScenesLocation = (std::filesystem::exists(currentUserScenesLocation))
					     ? std::move(currentUserScenesLocation)
					     : std::move(defaultUserScenesLocation);
		userProfilesLocation = (std::filesystem::exists(currentUserProfilesLocation))
					       ? std::move(currentUserProfilesLocation)
					       : std::move(defaultUserProfilesLocation);
		userPluginManagerSettingsLocation = (std::filesystem::exists(currentUserPluginManagerLocation))
							    ? std::move(currentUserPluginManagerLocation)
							    : std::move(defaultPluginManagerLocation);
	}

	bool userConfigResult = InitUserConfig(userConfigLocation, lastVersion);

	return userConfigResult;
}

bool OBSApp::InitUserConfig(std::filesystem::path &configLocation, uint32_t lastVersion)
{
	const std::string userConfigFile = configLocation.u8string() + "/" OBS_CONFIG_DIRECTORY "/user.ini";

	int errorCode = userConfig.Open(userConfigFile.c_str(), CONFIG_OPEN_ALWAYS);

	if (errorCode != CONFIG_SUCCESS) {
		OBSErrorBox(nullptr, "Failed to open user.ini: %d", errorCode);
		return false;
	}

	MigrateLegacySettings(lastVersion);
	InitUserConfigDefaults();

	return true;
}

void OBSApp::MigrateLegacySettings(const uint32_t lastVersion)
{
	bool hasUserConfigChanges = false;
	bool hasGlobalConfigChanges = false;

	const uint32_t v19 = MAKE_SEMANTIC_VERSION(19, 0, 0);
	const uint32_t v21 = MAKE_SEMANTIC_VERSION(21, 0, 0);
	const uint32_t v23 = MAKE_SEMANTIC_VERSION(23, 0, 0);
	const uint32_t v24 = MAKE_SEMANTIC_VERSION(24, 0, 0);
	const uint32_t v24_1 = MAKE_SEMANTIC_VERSION(24, 1, 0);

	const map<uint32_t, string> defaultsMap{
		{{v19, "Pre19Defaults"}, {v21, "Pre21Defaults"}, {v23, "Pre23Defaults"}, {v24_1, "Pre24.1Defaults"}}};

	for (auto &[version, configKey] : defaultsMap) {
		if (!config_has_user_value(userConfig, "General", configKey.c_str())) {
			bool useOldDefaults = lastVersion && lastVersion < version;
			config_set_bool(userConfig, "General", configKey.c_str(), useOldDefaults);

			hasUserConfigChanges = true;
		}
	}

	if (config_has_user_value(userConfig, "BasicWindow", "MultiviewLayout")) {
		const char *layout = config_get_string(userConfig, "BasicWindow", "MultiviewLayout");

		bool layoutUpdated = UpdatePre22MultiviewLayout(layout);

		hasUserConfigChanges = hasUserConfigChanges | layoutUpdated;
	}

	if (lastVersion && lastVersion < v24) {
		bool disableHotkeysInFocus = config_get_bool(userConfig, "General", "DisableHotkeysInFocus");

		if (disableHotkeysInFocus) {
			config_set_string(userConfig, "General", "HotkeyFocusType", "DisableHotkeysInFocus");
		}

		hasUserConfigChanges = true;
	}

	if (lastVersion && lastVersion < MAKE_SEMANTIC_VERSION(33, 0, 0)) {
		bool migratedUserSettings = config_has_user_value(appConfig, "General", "Pre33.0Migrated");

		if (!migratedUserSettings) {
			double currentSnapDistance = config_get_double(userConfig, "BasicWindow", "SnapDistance");

			if (currentSnapDistance == 10.0) {
				double newDefaultSnapDistance = kDefaultSnapDistance;
				config_set_double(userConfig, "BasicWindow", "SnapDistance", newDefaultSnapDistance);
			}

			config_set_bool(appConfig, "General", "Pre33.0Migrated", true);

			hasUserConfigChanges = true;
			hasGlobalConfigChanges = true;
		}
	}

	if (hasUserConfigChanges) {
		userConfig.SaveSafe("tmp");
	}

	if (hasGlobalConfigChanges) {
		appConfig.SaveSafe("tmp");
	}
}

static constexpr string_view OBSGlobalIniPath = "/" OBS_CONFIG_DIRECTORY "/global.ini";
static constexpr string_view OBSUserIniPath = "/" OBS_CONFIG_DIRECTORY "/user.ini";

bool OBSApp::MigrateGlobalSettings()
{
	char path[512];

	int len = GetAppConfigPath(path, sizeof(path), nullptr);
	if (len <= 0) {
		OBSErrorBox(nullptr, "Unable to get global configuration path.");
		return false;
	}

	std::string legacyConfigFileString;
	legacyConfigFileString.reserve(strlen(path) + OBSGlobalIniPath.size());
	legacyConfigFileString.append(path).append(OBSGlobalIniPath);

	const std::filesystem::path legacyGlobalConfigFile = std::filesystem::u8path(legacyConfigFileString);

	std::string configFileString;
	configFileString.reserve(strlen(path) + OBSUserIniPath.size());
	configFileString.append(path).append(OBSUserIniPath);

	const std::filesystem::path userConfigFile = std::filesystem::u8path(configFileString);

	if (std::filesystem::exists(userConfigFile)) {
		OBSErrorBox(nullptr,
			    "Unable to migrate global configuration - user configuration file already exists.");
		return false;
	}

	try {
		std::filesystem::copy(legacyGlobalConfigFile, userConfigFile);
	} catch (const std::filesystem::filesystem_error &) {
		OBSErrorBox(nullptr, "Unable to migrate global configuration - copy failed.");
		return false;
	}

	return true;
}

bool OBSApp::InitLocale()
{
	ProfileScope("OBSApp::InitLocale");

	const char *lang = config_get_string(userConfig, "General", "Language");
	bool userLocale = config_has_user_value(userConfig, "General", "Language");
	if (!userLocale || !lang || lang[0] == '\0') {
		lang = DEFAULT_LANG;
	}

	locale = lang;

	// set basic default application locale
	if (!locale.empty()) {
		QLocale::setDefault(QLocale(QString::fromStdString(locale).replace('-', '_')));
	}

	string englishPath;
	if (!GetDataFilePath("locale/" DEFAULT_LANG ".ini", englishPath)) {
		OBSErrorBox(NULL, "Failed to find locale/" DEFAULT_LANG ".ini");
		return false;
	}

	textLookup = text_lookup_create(englishPath.c_str());
	if (!textLookup) {
		OBSErrorBox(NULL, "Failed to create locale from file '%s'", englishPath.c_str());
		return false;
	}

	bool defaultLang = astrcmpi(lang, DEFAULT_LANG) == 0;

	if (userLocale && defaultLang) {
		return true;
	}

	if (!userLocale && defaultLang) {
		for (auto &locale_ : GetPreferredLocales()) {
			if (locale_ == lang) {
				return true;
			}

			stringstream file;
			file << "locale/" << locale_ << ".ini";

			string path;
			if (!GetDataFilePath(file.str().c_str(), path)) {
				continue;
			}

			if (!text_lookup_add(textLookup, path.c_str())) {
				continue;
			}

			blog(LOG_INFO, "Using preferred locale '%s'", locale_.c_str());
			locale = locale_;

			// set application default locale to the new chosen one
			if (!locale.empty()) {
				QLocale::setDefault(QLocale(QString::fromStdString(locale).replace('-', '_')));
			}

			return true;
		}

		return true;
	}

	stringstream file;
	file << "locale/" << lang << ".ini";

	string path;
	if (GetDataFilePath(file.str().c_str(), path)) {
		if (!text_lookup_add(textLookup, path.c_str())) {
			blog(LOG_ERROR, "Failed to add locale file '%s'", path.c_str());
		}
	} else {
		blog(LOG_ERROR, "Could not find locale file '%s'", file.str().c_str());
	}

	return true;
}

#ifdef OBS_PLATFORM_HAS_UPDATE_BRANCHES
void ParseBranchesJson(const std::string &jsonString, vector<UpdateBranch> &out, std::string &error)
{
	JsonBranches branches;

	try {
		nlohmann::json json = nlohmann::json::parse(jsonString);
		branches = json.get<JsonBranches>();
	} catch (nlohmann::json::exception &e) {
		error = e.what();
		return;
	}

	for (const JsonBranch &json_branch : branches) {
		if (!IsUpdateBranchSupported(json_branch.windows, json_branch.macos)) {
			continue;
		}

		UpdateBranch branch = {
			QString::fromStdString(json_branch.name),
			QString::fromStdString(json_branch.display_name),
			QString::fromStdString(json_branch.description),
			json_branch.enabled,
			json_branch.visible,
		};
		out.push_back(branch);
	}
}

bool LoadBranchesFile(vector<UpdateBranch> &out)
{
	string error;
	string branchesText;

	BPtr<char> branchesFilePath = GetAppConfigPathPtr("obs-studio/updates/branches.json");

	QFile branchesFile(branchesFilePath.Get());
	if (!branchesFile.open(QIODevice::ReadOnly)) {
		error = "Opening file failed.";
		goto fail;
	}

	branchesText = branchesFile.readAll().toStdString();
	if (branchesText.empty()) {
		error = "File empty.";
		goto fail;
	}

	ParseBranchesJson(branchesText, out, error);
	if (error.empty()) {
		return !out.empty();
	}

fail:
	blog(LOG_WARNING, "Loading branches from file failed: %s", error.c_str());
	return false;
}
#endif

void OBSApp::SetBranchData(const string &data)
{
#ifdef OBS_PLATFORM_HAS_UPDATE_BRANCHES
	string error;
	vector<UpdateBranch> result;

	ParseBranchesJson(data, result, error);

	if (!error.empty()) {
		blog(LOG_WARNING, "Reading branches JSON response failed: %s", error.c_str());
		return;
	}

	if (!result.empty()) {
		updateBranches = result;
	}

	branches_loaded = true;
#else
	UNUSED_PARAMETER(data);
#endif
}

std::vector<UpdateBranch> OBSApp::GetBranches()
{
	vector<UpdateBranch> out;
	/* Always ensure the default branch exists */
	out.push_back(UpdateBranch{"stable", "", "", true, true});

#ifdef OBS_PLATFORM_HAS_UPDATE_BRANCHES
	if (!branches_loaded) {
		vector<UpdateBranch> result;
		if (LoadBranchesFile(result)) {
			updateBranches = result;
		}

		branches_loaded = true;
	}

	/* OBS-Studio-Rust's opt-in nightly branch (#1759): the daily builds'
	 * feed on the continuous release. Built in, so it is offered before
	 * any certified branches.json lists it. */
	out.push_back(UpdateBranch{OBS_RUST_NIGHTLY_BRANCH, "Nightly", "", true, true});
#endif

	/* Copy additional branches to result (if any) */
	for (const UpdateBranch &branch : updateBranches) {
		bool listed = false;
		for (const UpdateBranch &known : out) {
			listed = listed || known.name == branch.name;
		}
		if (!listed) {
			out.push_back(branch);
		}
	}

	return out;
}

OBSApp::OBSApp(int &argc, char **argv, profiler_name_store_t *store)
	: QApplication(argc, argv),
	  profilerNameStore(store),
	  appLaunchUUID_(QUuid::createUuid())
{
	nativeEventFilter_ = std::make_unique<OBS::NativeEventFilter>();
	installNativeEventFilter(nativeEventFilter_.get());

	/* fix float handling */
#if defined(Q_OS_UNIX)
	if (!setlocale(LC_NUMERIC, "C")) {
		blog(LOG_WARNING, "Failed to set LC_NUMERIC to C locale");
	}
#endif

	ConnectPlatformAppSignals(this);
	connect(qApp, &QGuiApplication::commitDataRequest, this, &OBSApp::commitData, Qt::DirectConnection);

#ifndef OBS_RUST_VERSION
	InitCrashHandler();
#endif

	sleepInhibitor = os_inhibit_sleep_create("OBS Video/audio");

	SetApplicationIcon();

	setDesktopFileName(OBS_DESKTOP_ID);

	pluginManager_ = std::make_unique<OBS::PluginManager>();
}

void OBSApp::InitCrashHandler()
{
	if (multi) {
		crashHandler_ = std::make_unique<OBS::CrashHandler>();
	} else {
		crashHandler_ = std::make_unique<OBS::CrashHandler>(appLaunchUUID_);
	}
}

OBSApp::~OBSApp()
{
	if (libobs_initialized) {
		applicationShutdown();
	}
	/* Before nativeEventFilter_ is freed: QCoreApplication outlives it. */
	removeNativeEventFilter(nativeEventFilter_.get());
};

static void move_basic_to_profiles(void)
{
	char path[512];

	if (GetAppConfigPath(path, 512, "obs-studio/basic") <= 0) {
		return;
	}

	const std::filesystem::path basicPath = std::filesystem::u8path(path);

	if (!std::filesystem::exists(basicPath)) {
		return;
	}

	const std::filesystem::path profilesPath =
		App()->userProfilesLocation / std::filesystem::u8path(OBS_CONFIG_DIRECTORY "/basic/profiles");

	if (std::filesystem::exists(profilesPath)) {
		return;
	}

	try {
		std::filesystem::create_directories(profilesPath);
	} catch (const std::filesystem::filesystem_error &error) {
		blog(LOG_ERROR, "Failed to create profiles directory for migration from basic profile\n%s",
		     error.what());
		return;
	}

	const std::filesystem::path newProfilePath = profilesPath / std::filesystem::u8path(Str("Untitled"));

	for (auto &entry : std::filesystem::directory_iterator(basicPath)) {
		if (entry.is_directory()) {
			continue;
		}

		if (entry.path().filename().u8string() == "scenes.json") {
			continue;
		}

		if (!std::filesystem::exists(newProfilePath)) {
			try {
				std::filesystem::create_directory(newProfilePath);
			} catch (const std::filesystem::filesystem_error &error) {
				blog(LOG_ERROR, "Failed to create profile directory for 'Untitled'\n%s", error.what());
				return;
			}
		}

		const filesystem::path destinationFile = newProfilePath / entry.path().filename();

		const auto copyOptions = std::filesystem::copy_options::overwrite_existing;

		try {
			std::filesystem::copy(entry.path(), destinationFile, copyOptions);
		} catch (const std::filesystem::filesystem_error &error) {
			blog(LOG_ERROR, "Failed to copy basic profile file '%s' to new profile 'Untitled'\n%s",
			     entry.path().filename().u8string().c_str(), error.what());

			return;
		}
	}
}

static void move_basic_to_scene_collections(void)
{
	char path[512];

	if (GetAppConfigPath(path, 512, "obs-studio/basic") <= 0) {
		return;
	}

	migrate_basic_scene_collection(std::filesystem::u8path(path),
				       App()->userScenesLocation /
					       std::filesystem::u8path(OBS_CONFIG_DIRECTORY "/basic/scenes"),
				       Str("Untitled"));
}

void OBSApp::AppInit()
{
	ProfileScope("OBSApp::AppInit");

	GpuStatus::Prefetch();

	QAccessible::installFactory(alignmentSelectorFactory);

	if (!InitializeAppIdentity()) {
		throw "Failed to initialize application identity";
	}
	if (!MigrateRustConfiguration()) {
		throw "Failed to import settings into the independent Rust configuration";
	}
#ifdef OBS_RUST_VERSION
	InitCrashHandler();
#endif
	if (!MakeUserDirs()) {
		throw "Failed to create required user directories";
	}
	if (!InitGlobalConfig()) {
		throw "Failed to initialize global config";
	}
	if (!InitLocale()) {
		throw "Failed to load locale";
	}
	if (!InitTheme()) {
		throw "Failed to load theme";
	}

	config_set_default_string(userConfig, "Basic", "Profile", Str("Untitled"));
	config_set_default_string(userConfig, "Basic", "ProfileDir", Str("Untitled"));
	config_set_default_string(userConfig, "Basic", "SceneCollection", Str("Untitled"));
	config_set_default_string(userConfig, "Basic", "SceneCollectionFile", Str("Untitled"));
	config_set_default_bool(userConfig, "Basic", "ConfigOnNewProfile", true);

	const std::string_view profileName{config_get_string(userConfig, "Basic", "Profile")};

	if (profileName.empty()) {
		config_set_string(userConfig, "Basic", "Profile", Str("Untitled"));
		config_set_string(userConfig, "Basic", "ProfileDir", Str("Untitled"));
	}

	const std::string_view sceneCollectionName{config_get_string(userConfig, "Basic", "SceneCollection")};

	if (sceneCollectionName.empty()) {
		config_set_string(userConfig, "Basic", "SceneCollection", Str("Untitled"));
		config_set_string(userConfig, "Basic", "SceneCollectionFile", Str("Untitled"));
	}

	ApplyPlatformAppSettings(appConfig);

	UpdateHotkeyFocusSetting(false);

	move_basic_to_profiles();
	move_basic_to_scene_collections();

	if (!MakeUserProfileDirs()) {
		throw "Failed to create profile directories";
	}
}

void OBSApp::checkForUncleanShutdown()
{
	bool hasUncleanShutdown = crashHandler_->hasUncleanShutdown();
	bool hasNewCrashLog = crashHandler_->hasNewCrashLog();

	const UncleanLaunchDecision decision = decideUncleanLaunch(hasUncleanShutdown, disable_shutdown_check);

	if (decision.logSkippedPrompt) {
		blog(LOG_WARNING, "Crash or unclean shutdown detected; prompt skipped (--disable-shutdown-check), "
				  "launching normally");
	}

	if (decision.showPrompt) {
		UncleanLaunchAction launchAction = handleUncleanShutdown(hasNewCrashLog);

		safe_mode = launchAction.useSafeMode;

		if (launchAction.sendCrashReport) {
			crashHandler_->uploadLastCrashLog();
		}
	}
}

const char *OBSApp::GetRenderModule() const
{
	const char *renderer = config_get_string(appConfig, "Video", "Renderer");

	return GetPlatformRenderModule(renderer);
}

static bool StartupOBS(const char *locale, profiler_name_store_t *store)
{
	char path[512];

	if (GetAppConfigPath(path, sizeof(path), "obs-studio/plugin_config") <= 0) {
		return false;
	}

	return obs_startup(locale, path, store);
}

inline void OBSApp::ResetHotkeyState(bool inFocus)
{
	obs_hotkey_enable_background_press((inFocus && enableHotkeysInFocus) || (!inFocus && enableHotkeysOutOfFocus));
}

void OBSApp::UpdateHotkeyFocusSetting(bool resetState)
{
	enableHotkeysInFocus = true;
	enableHotkeysOutOfFocus = true;

	const char *hotkeyFocusType = config_get_string(userConfig, "General", "HotkeyFocusType");

	if (astrcmpi(hotkeyFocusType, "DisableHotkeysInFocus") == 0) {
		enableHotkeysInFocus = false;
	} else if (astrcmpi(hotkeyFocusType, "DisableHotkeysOutOfFocus") == 0) {
		enableHotkeysOutOfFocus = false;
	}

	if (resetState) {
		ResetHotkeyState(applicationState() == Qt::ApplicationActive);
	}
}

void OBSApp::DisableHotkeys()
{
	enableHotkeysInFocus = false;
	enableHotkeysOutOfFocus = false;
	ResetHotkeyState(applicationState() == Qt::ApplicationActive);
}

void OBSApp::Exec(VoidFunc func)
{
	func();
}

static void ui_task_handler(obs_task_t task, void *param, bool wait)
{
	auto doTask = [=]() {
		/* to get clang-format to behave */
		task(param);
	};
	QMetaObject::invokeMethod(App(), &OBSApp::Exec, wait ? WaitConnection() : Qt::AutoConnection, doTask);
}

bool OBSApp::OBSInit()
{
	ProfileScope("OBSApp::OBSInit");

	qRegisterMetaType<VoidFunc>("VoidFunc");

	InitPlatformDisplay();

	if (!StartupOBS(locale.c_str(), GetProfilerNameStore())) {
		return false;
	}

	libobs_initialized = true;

	obs_set_ui_task_handler(ui_task_handler);

	if (PlatformSupportsBrowserHWAccel()) {
		bool browserHWAccel = config_get_bool(appConfig, "General", "BrowserHWAccel");

		OBSDataAutoRelease settings = obs_data_create();
		obs_data_set_bool(settings, "BrowserHWAccel", browserHWAccel);
		obs_apply_private_data(settings);

		blog(LOG_INFO, "Current Date/Time: %s", CurrentDateTimeString().c_str());

		blog(LOG_INFO, "Browser Hardware Acceleration: %s", browserHWAccel ? "true" : "false");
	}
	LogPlatformSettings(userConfig);

	blog(LOG_INFO, "Qt Version: %s (runtime), %s (compiled)", qVersion(), QT_VERSION_STR);
	blog(LOG_INFO, "Portable mode: %s", portable_mode ? "true" : "false");

	if (safe_mode) {
		blog(LOG_WARNING, "Safe Mode enabled.");
	} else if (disable_3p_plugins) {
		blog(LOG_WARNING, "Third-party plugins disabled.");
	}

	setQuitOnLastWindowClosed(false);

	thumbnailManager = new ThumbnailManager(this);

	// Release the partially initialized window if OBSInit throws.
	auto initializingWindow = std::make_unique<OBSBasic>();
	mainWindow = initializingWindow.get();

	mainWindow->setAttribute(Qt::WA_DeleteOnClose, true);

	mainWindow->OBSInit();
	initializingWindow.release();

	connect(OBSBasic::Get(), &OBSBasic::mainWindowClosed, crashHandler_.get(),
		&OBS::CrashHandler::applicationShutdownHandler);

	connect(this, &QGuiApplication::applicationStateChanged, this,
		[this](Qt::ApplicationState state) { ResetHotkeyState(state == Qt::ApplicationActive); });
	ResetHotkeyState(applicationState() == Qt::ApplicationActive);

	connect(crashHandler_.get(), &OBS::CrashHandler::crashLogUploadFailed, this,
		[this](const QString &errorMessage) {
			emit this->logUploadFailed(OBS::LogFileType::CrashLog, errorMessage);
		});

	connect(crashHandler_.get(), &OBS::CrashHandler::crashLogUploadFinished, this, [this](const QString &filePath) {
		/* The crash log stays local (#1731): show its folder. */
		openCrashLogDirectory();
		emit this->logUploadFinished(OBS::LogFileType::CrashLog, filePath);
	});

	return true;
}

/* Product name and version for display: "OBS <version>", or for the Rust build
 * (ENABLE_RUST_LIBOBS) "OBS-Studio-Rust <rust version> (based on OBS <version>)". */
string OBSApp::GetProductString(bool platform) const
{
#ifdef OBS_RUST_VERSION
	const char *rust_version = OBS_RUST_VERSION;
#else
	const char *rust_version = nullptr;
#endif
	string version = GetVersionString(platform);
	int len = obs_product_string(nullptr, 0, version.c_str(), rust_version);
	string product(len > 0 ? (size_t)len : 0, '\0');
	obs_product_string(product.data(), product.size() + 1, version.c_str(), rust_version);
	return product;
}

string OBSApp::GetVersionString(bool platform) const
{
	stringstream ver;

	ver << obs_get_version_string();

	if (platform) {
		ver << " (" << GetPlatformVersionName() << ")";
	}

	return ver.str();
}

bool OBSApp::IsPortableMode()
{
	return portable_mode;
}

bool OBSApp::IsUpdaterDisabled()
{
	return opt_disable_updater;
}

bool OBSApp::IsMissingFilesCheckDisabled()
{
	return opt_disable_missing_files_check;
}

const char *OBSApp::InputAudioSource() const
{
	return GetPlatformInputAudioSource();
}

const char *OBSApp::OutputAudioSource() const
{
	return GetPlatformOutputAudioSource();
}

const char *OBSApp::GetLastLog() const
{
	return lastLogFile.c_str();
}

const char *OBSApp::GetCurrentLog() const
{
	return currentLogFile.c_str();
}

void OBSApp::openCrashLogDirectory() const
{
	std::filesystem::path crashLogDirectory = crashHandler_->getCrashLogDirectory();

	if (crashLogDirectory.empty()) {
		return;
	}

	QString crashLogDirectoryString = QString::fromStdString(crashLogDirectory.u8string());

	QUrl crashLogDirectoryURL = QUrl::fromLocalFile(crashLogDirectoryString);
	QDesktopServices::openUrl(crashLogDirectoryURL);
}

void OBSApp::uploadLastAppLog() const
{
	OBSBasic *basicWindow = static_cast<OBSBasic *>(GetMainWindow());

	basicWindow->UploadLog("obs-studio/logs", GetLastLog(), OBS::LogFileType::LastAppLog);
}

void OBSApp::uploadCurrentAppLog() const
{
	OBSBasic *basicWindow = static_cast<OBSBasic *>(GetMainWindow());

	basicWindow->UploadLog("obs-studio/logs", GetCurrentLog(), OBS::LogFileType::CurrentAppLog);
}

void OBSApp::uploadLastCrashLog()
{
	crashHandler_->uploadLastCrashLog();
}

OBS::LogFileState OBSApp::getLogFileState(OBS::LogFileType type) const
{
	switch (type) {
	case OBS::LogFileType::CrashLog: {
		bool hasNewCrashLog = crashHandler_->hasNewCrashLog();

		return (hasNewCrashLog) ? OBS::LogFileState::New : OBS::LogFileState::Uploaded;
	}
	case OBS::LogFileType::CurrentAppLog:
	case OBS::LogFileType::LastAppLog:
		return OBS::LogFileState::New;
	default:
		return OBS::LogFileState::NoState;
	}
}

bool OBSApp::TranslateString(const char *lookupVal, const char **out) const
{
	for (obs_frontend_translate_ui_cb cb : translatorHooks) {
		if (cb(lookupVal, out)) {
			return true;
		}
	}

	return text_lookup_getstr(App()->GetTextLookup(), lookupVal, out);
}

QStyle *OBSApp::GetInvisibleCursorStyle()
{
	if (!invisibleCursorStyle) {
		invisibleCursorStyle = std::make_unique<OBSInvisibleCursorProxyStyle>();
	}
	return invisibleCursorStyle.get();
}

// Global handler to receive all QEvent::Show events so we can apply
// display affinity on any newly created windows and dialogs without
// caring where they are coming from (e.g. plugins).
bool OBSApp::notify(QObject *receiver, QEvent *e)
{
	QWidget *w;
	QWindow *window;
	int windowType;

	if (!receiver->isWidgetType()) {
		goto skip;
	}

	if (e->type() != QEvent::Show) {
		goto skip;
	}

	w = qobject_cast<QWidget *>(receiver);

	if (!w->isWindow()) {
		goto skip;
	}

	window = w->windowHandle();
	if (!window) {
		goto skip;
	}

	windowType = window->flags() & Qt::WindowType::WindowType_Mask;

	if (windowType == Qt::WindowType::Dialog || windowType == Qt::WindowType::Window ||
	    windowType == Qt::WindowType::Tool) {
		OBSBasic *main = OBSBasic::Get();
		if (main) {
			main->SetDisplayAffinity(window);
		}
	}

skip:
	return QApplication::notify(receiver, e);
}

string GenerateTimeDateFilename(const char *extension, bool noSpace)
{
	time_t now = time(0);
	char file[256] = {};
	struct tm cur_time;

	PlatformLocalTime(&now, &cur_time);
	snprintf(file, sizeof(file), "%d-%02d-%02d%c%02d-%02d-%02d.%s", cur_time.tm_year + 1900, cur_time.tm_mon + 1,
		 cur_time.tm_mday, noSpace ? '_' : ' ', cur_time.tm_hour, cur_time.tm_min, cur_time.tm_sec, extension);

	return string(file);
}

string GenerateSpecifiedFilename(const char *extension, bool noSpace, const char *format)
{
	BPtr<char> filename = os_generate_formatted_filename(extension, !noSpace, format);
	return string(filename);
}

static void ensure_directory_exists(string &path)
{
	replace(path.begin(), path.end(), '\\', '/');

	size_t last = path.rfind('/');
	if (last == string::npos) {
		return;
	}

	string directory = path.substr(0, last);
	os_mkdirs(directory.c_str());
}

string GetFormatString(const char *format, const char *prefix, const char *suffix)
{
	char *f = output_filename_format(format, prefix, suffix);
	string result = f ? f : "";
	free(f);
	return result;
}

string GetFormatExt(const char *container)
{
	string ext = container;
	if (ext == "fragmented_mp4" || ext == "hybrid_mp4") {
		ext = "mp4";
	} else if (ext == "fragmented_mov" || ext == "hybrid_mov") {
		ext = "mov";
	} else if (ext == "hls") {
		ext = "m3u8";
	} else if (ext == "mpegts") {
		ext = "ts";
	}

	return ext;
}

string GetOutputFilename(const char *path, const char *container, bool noSpace, bool overwrite, const char *format,
			 bool *reserved)
{
	if (reserved) {
		*reserved = false;
	}

	OBSBasic *main = OBSBasic::Get();

	os_dir_t *dir = path && path[0] ? os_opendir(path) : nullptr;

	if (!dir) {
		if (main->isVisible()) {
			OBSMessageBox::warning(main, QTStr("Output.BadPath.Title"), QTStr("Output.BadPath.Text"));
		} else {
			main->SysTrayNotify(QTStr("Output.BadPath.Text"), QSystemTrayIcon::Warning);
		}
		return "";
	}

	os_closedir(dir);

	string strPath;
	strPath += path;

	char lastChar = strPath.back();
	if (lastChar != '/' && lastChar != '\\') {
		strPath += "/";
	}

	string ext = GetFormatExt(container);
	strPath += GenerateSpecifiedFilename(ext.c_str(), noSpace, format);
	ensure_directory_exists(strPath);

	/* Creates the file (unless overwrite is set), so another output
	 * starting in the same second gets its own name. */
	BPtr<char> reservedPath = output_filename_reserve(strPath.c_str(), noSpace, overwrite, reserved);
	return reservedPath ? string(reservedPath) : strPath;
}

vector<pair<string, string>> GetLocaleNames()
{
	string path;
	if (!GetDataFilePath("locale.ini", path)) {
		throw "Could not find locale.ini path";
	}

	ConfigFile ini;
	if (ini.Open(path.c_str(), CONFIG_OPEN_EXISTING) != 0) {
		throw "Could not open locale.ini";
	}

	size_t sections = config_num_sections(ini);

	vector<pair<string, string>> names;
	names.reserve(sections);
	for (size_t i = 0; i < sections; i++) {
		const char *tag = config_get_section(ini, i);
		const char *name = config_get_string(ini, tag, "Name");
		names.emplace_back(tag, name);
	}

	return names;
}

int GetAppConfigPath(char *path, size_t size, const char *name)
{
#ifdef OBS_RUST_VERSION
	const std::string relativePath = AppConfigRelativePath(name);
	name = relativePath.c_str();
#endif
	if (IsPortableModeAllowed() && portable_mode) {
		if (name && *name) {
			return snprintf(path, size, "%s/config/%s", GetPortableConfigBasePath(), name);
		} else {
			return snprintf(path, size, "%s/config", GetPortableConfigBasePath());
		}
	} else {
		return os_get_config_path(path, size, name);
	}
}

char *GetAppConfigPathPtr(const char *name)
{
#ifdef OBS_RUST_VERSION
	const std::string relativePath = AppConfigRelativePath(name);
	name = relativePath.c_str();
#endif
	if (IsPortableModeAllowed() && portable_mode) {
		char path[512];

		if (snprintf(path, sizeof(path), "%s/config/%s", GetPortableConfigBasePath(), name) > 0) {
			return bstrdup(path);
		} else {
			return NULL;
		}
	} else {
		return os_get_config_path_ptr(name);
	}
}

int GetProgramDataPath(char *path, size_t size, const char *name)
{
	return os_get_program_data_path(path, size, name);
}

char *GetProgramDataPathPtr(const char *name)
{
	return os_get_program_data_path_ptr(name);
}

bool GetFileSafeName(const char *name, std::string &file)
{
	size_t base_len = strlen(name);
	size_t len = os_utf8_to_wcs(name, base_len, nullptr, 0);
	std::wstring wfile;

	if (!len) {
		return false;
	}

	wfile.resize(len);
	os_utf8_to_wcs(name, base_len, &wfile[0], len + 1);

	for (size_t i = wfile.size(); i > 0; i--) {
		size_t im1 = i - 1;

		if (iswspace(wfile[im1])) {
			wfile[im1] = '_';
		} else if (wfile[im1] != '_' && !iswalnum(wfile[im1])) {
			wfile.erase(im1, 1);
		}
	}

	if (wfile.size() == 0) {
		wfile = L"characters_only";
	}

	len = os_wcs_to_utf8(wfile.c_str(), wfile.size(), nullptr, 0);
	if (!len) {
		return false;
	}

	file.resize(len);
	os_wcs_to_utf8(wfile.c_str(), wfile.size(), &file[0], len + 1);
	return true;
}

bool GetClosestUnusedFileName(std::string &path, const char *extension)
{
	size_t len = path.size();
	if (extension) {
		path += ".";
		path += extension;
	}

	if (!os_file_exists(path.c_str())) {
		return true;
	}

	int index = 1;

	do {
		path.resize(len);
		path += std::to_string(++index);
		if (extension) {
			path += ".";
			path += extension;
		}
	} while (os_file_exists(path.c_str()));

	return true;
}

bool WindowPositionValid(QRect rect)
{
	for (QScreen *screen : QGuiApplication::screens()) {
		if (screen->availableGeometry().intersects(rect)) {
			return true;
		}
	}
	return false;
}

void OBSApp::processSigInt()
{
	if (!ConsumePlatformAppSignal(PlatformAppSignal::Interrupt)) {
		return;
	}

	if (PlatformSavesBeforeSignalQuit()) {
		OBSBasic *main = OBSBasic::Get();
		if (main) {
			main->saveAll();
			main->close();
		}
	} else {
		quit();
	}
}

void OBSApp::processSigTerm()
{
	if (!ConsumePlatformAppSignal(PlatformAppSignal::Terminate)) {
		return;
	}

	OBSBasic *main = OBSBasic::Get();
	if (main) {
		if (PlatformSavesBeforeSignalQuit()) {
			main->saveAll();
		}
		/* systemctl stop, logind and kill send SIGTERM; nobody is there to
		 * answer "outputs are active, exit anyway?", and while that dialog
		 * waits the recording is cut off when the sender gives up. Quit as
		 * if it had been answered: closeWindow() stops the outputs and
		 * waits for the recording to finish. */
		main->closeWithoutPrompt();
	}

	/* A modal dialog that is waiting (for example one the main window opens
	 * during its startup) runs its own event loop, and quit() does not end
	 * it; before the main event loop has started, quit() does nothing at
	 * all. Dismiss the dialogs, then quit once their loops have returned to
	 * the code that opened them; main() quits after a startup that SIGTERM
	 * interrupted. */
	if (activeModalWidget()) {
		quitRequested_ = true;
		for (int i = 0; i < 16; i++) {
			QWidget *modal = activeModalWidget();
			if (!modal) {
				break;
			}
			if (QDialog *dialog = qobject_cast<QDialog *>(modal)) {
				dialog->reject();
			} else {
				modal->close();
			}
		}
		QTimer::singleShot(0, this, &OBSApp::quit);
		return;
	}
	quit();
}

void OBSApp::processSigAbrt()
{
	if (!ConsumePlatformAppSignal(PlatformAppSignal::Abort)) {
		return;
	}

	if (PlatformSavesBeforeSignalQuit()) {
		OBSBasic *main = OBSBasic::Get();
		if (main) {
			main->saveAll();
		}
	}
	quit();
}

void OBSApp::processSigQuit()
{
	if (!ConsumePlatformAppSignal(PlatformAppSignal::Quit)) {
		return;
	}

	if (PlatformSavesBeforeSignalQuit()) {
		OBSBasic *main = OBSBasic::Get();
		if (main) {
			main->saveAll();
		}
	}
	quit();
}

void OBSApp::commitData(QSessionManager &manager)
{
	OBSBasic *main = OBSBasic::Get();
	if (main) {
		main->saveAll();

		if (manager.allowsInteraction() && main->shouldPromptForClose()) {
			manager.cancel();
		}
	}
}

void OBSApp::applicationShutdown() noexcept
{
	RevertPlatformAppSettings(appConfig);

	DisconnectPlatformAppSignals();

	os_inhibit_sleep_set_active(sleepInhibitor, false);
	os_inhibit_sleep_destroy(sleepInhibitor);

	if (libobs_initialized) {
		obs_shutdown();
		libobs_initialized = false;
	}
}

void OBSApp::addLogLine(int logLevel, const QString &message)
{
	emit logLineAdded(logLevel, message);
}

void OBSApp::loadAppModules()
{
	using PluginMode = OBS::PluginManager::Mode;
	PluginMode mode = (disable_3p_plugins || safe_mode) ? PluginMode::CoreOnly : PluginMode::Full;
	pluginManager_->setPluginMode(mode);

	pluginManager_->loadAllPlugins(portable_mode);
}

void OBSApp::handlePluginLoadState()
{
	using PluginState = OBS::PluginManager::State;
	PluginState loadState = pluginManager_->loadState();

	if (loadState != PluginState::Success) {
		PluginFailureAction action = handlePluginFailure();

		if (action == PluginFailureAction::OpenPluginManager) {
			pluginManagerOpenDialog();
		}
	}
}

void OBSApp::pluginManagerOpenDialog()
{
	pluginManager_->open();
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef DEFAULT_LANG
