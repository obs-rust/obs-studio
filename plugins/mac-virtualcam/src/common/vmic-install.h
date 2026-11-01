#pragma once

/* OBS Virtual Microphone on macOS (obs-rust/obs-studio#3341): whether the
 * HAL plug-in in /Library/Audio/Plug-Ins/HAL needs installing or updating
 * from the copy inside OBS.app. Plain C so the cmocka test runs on every
 * host. */

#include <stddef.h>
#include <string.h>

enum vmic_driver_action {
	VMIC_DRIVER_OK,          /* installed and current */
	VMIC_DRIVER_INSTALL,     /* not installed; OBS.app has one */
	VMIC_DRIVER_UPDATE,      /* installed, but not the bundled version */
	VMIC_DRIVER_UNAVAILABLE, /* neither installed nor bundled */
};

/* Versions are CFBundleShortVersionString and CFBundleVersion; NULL means
 * that bundle is absent. A development build without a bundled copy keeps
 * whatever is installed. */
static inline enum vmic_driver_action vmic_driver_action(const char *installed_version, const char *installed_build,
							 const char *bundled_version, const char *bundled_build)
{
	(void)installed_version;
	(void)installed_build;
	(void)bundled_version;
	(void)bundled_build;
	return VMIC_DRIVER_OK;
}
