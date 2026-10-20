/* Test stand-in for frontend/OBSApp.hpp, for test_crash_log_upload: only what
 * utility/CrashHandler.cpp uses from it. No app configuration: the crash
 * handler then has no last uploaded crash log on record. */

#pragma once

#include <util/config-file.h>
#include <util/util.hpp>

#include <QString>

#include <string>

class OBSApp {
public:
	config_t *GetAppConfig() const { return nullptr; }
	std::string GetProductString(bool) const { return "OBS Studio"; }
};

OBSApp *App();

char *GetAppConfigPathPtr(const char *name);

/* Returns the lookup key itself, so the test can tell the strings apart. */
inline QString QTStr(const char *lookupVal)
{
	return QString::fromUtf8(lookupVal);
}
