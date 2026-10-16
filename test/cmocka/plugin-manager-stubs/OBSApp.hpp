/* Test stand-in for frontend/OBSApp.hpp, for test_plugin_manager: only what
 * frontend/plugin-manager/ uses from it. The plugin manager sources include it
 * as <OBSApp.hpp>, and this directory comes first on the test's include path. */

#pragma once

#include <utility/AppIdentity.hpp>

#include <obs-frontend-api.h>

#include <QApplication>
#include <QString>

#include <filesystem>

class OBSApp {
public:
	std::filesystem::path userPluginManagerSettingsLocation;
};

OBSApp *App();

/* Returns the lookup key itself, so the test can tell the strings apart. */
inline QString QTStr(const char *lookupVal)
{
	return QString::fromUtf8(lookupVal);
}
