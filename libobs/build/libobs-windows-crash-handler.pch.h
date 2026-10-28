/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.windows-crash-handler. C. The crash handler is built with
 * OBS_VERSION defined on the command line, so it must not see obsversion.h
 * (through obs-internal.h) the way libobs-windows.pch.h's files do.
 * Its implementation file includes this header and nothing else. */

#pragma once

#include <windows.h>
#include <time.h>
#include <dbghelp.h>
#include <shellapi.h>
#include <tlhelp32.h>
#include <inttypes.h>

#include "obs-config.h"
#include "util/dstr.h"
#include "util/platform.h"
#include "util/windows/win-version.h"

/* obs-win-crash-handler.c calls the crash report rules (obs-win-crash-report.c). */
#include "libobs/obs-win-crash-report.h"
