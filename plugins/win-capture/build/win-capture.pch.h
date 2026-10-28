/* Precompiled header (BUILD.md) for the plugins/win-capture unity groups
 * win-capture.windows, win-capture.window-capture-windows. C. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* app-helpers.c */
#include <windows.h>
#include <stdio.h>
#include <sddl.h> /* ConvertSidToStringSidW */
#include "plugins/win-capture/app-helpers.h"
#include "plugins/win-capture/nt-stuff.h"
#include "util/base.h"

/* audio-helpers.c */
#include "plugins/win-capture/audio-helpers.h"

#include <util/dstr.h>

/* compat-helpers.c */
#include <jansson.h>

#include <obs-module.h>
#include <util/platform.h>
#include <util/windows/window-helpers.h>

#include "plugins/win-capture/compat-helpers.h"
#include "plugins/win-capture/compat-format-ver.h"

/* cursor-capture.c */
#include <obs.h>
#include "plugins/win-capture/cursor-capture.h"

/* dc-capture.c */
#include "plugins/win-capture/dc-capture.h"

/* duplicator-monitor-capture.c */
#include "plugins/win-capture/monitor-identity.h"
#include <setupapi.h>

#include <util/threading.h>

#ifdef OBS_LEGACY
#include "../../libobs/util/platform.h"
#include "../../libobs-winrt/winrt-capture.h"
#else
#include <util/platform.h>
#include <winrt-capture.h>
#endif

/* game-capture-file-init.c */
#include <strsafe.h>
#include <shlobj.h>
#include <aclapi.h>
#include <sddl.h>
#include <util/windows/win-version.h>
#include <util/c99defs.h>
#include <util/base.h>

/* game-capture.c */
#include <inttypes.h>
#include <obs-hotkey.h>
#include <dxgi.h>
#include <util/sse-intrin.h>
#include <util/util_uint64.h>
#include <ipc-util/pipe.h>
#include <util/windows/obfuscate.h>
#include "inject-library.h"
#include "graphics-hook-info.h"
#include "graphics-hook-ver.h"

/* load-graphics-offsets.c */
#include <util/config-file.h>
#include <util/pipe.h>

/* nt-stuff.c */
#include <winternl.h>
#include <stdbool.h>

/* plugin-main.c */

#include <file-updater/file-updater.h>

#ifdef OBS_LEGACY
#include "compat-config.h"
#endif

/* window-capture.c */
#include <stdlib.h>

#ifdef OBS_LEGACY
#include "../../libobs/util/platform.h"
#include "../../libobs-winrt/winrt-capture.h"
#else
#include <util/platform.h>
#include <winrt-capture.h>
#endif
