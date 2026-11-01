#pragma once

/* The device rules coreaudio-enum-devices.c calls (coreaudio-device-rules.c),
 * declared here so every build of that file has them, the C tests' stand-in
 * of libobs-macos.pch.h included. */
#include "coreaudio-device-rules.h"

static bool success_(OSStatus stat, const char *func, const char *call)
{
	if (stat != noErr) {
		blog(LOG_WARNING, "%s: %s failed: %d", func, call, (int)stat);
		return false;
	}

	return true;
}

#define success(stat, call) success_(stat, __FUNCTION__, call)
