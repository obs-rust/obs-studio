/* Precompiled header (BUILD.md) for the libobs unity groups libobs.windows. C.
 * Every implementation file in them includes this header and nothing else: the includes and macro setup below are
 * theirs, in group and file order. */

#pragma once

/* audio-monitoring/win32/wasapi-enum-devices.c */
#include "obs-internal.h"

#include "audio-monitoring/win32/wasapi-output.h"

#include <propsys.h>

/* audio-monitoring/win32/wasapi-output.c */
#include "audio-monitoring/monitor-speaker-layout.h"
#include "audio-monitoring/monitor-video-sync.h"
#include "media-io/audio-resampler.h"
#include "util/deque.h"
#include "util/platform.h"
#include "util/darray.h"
#include "util/util_uint64.h"

/* obs-core-modules-loader-win.c */
#include "obs-core-modules.h"

#include <obs-internal.h>
#include <obs.h>
#include <util/dstr.h>
#include <util/platform.h>

/* obs-windows.c */
#include "util/windows/win-registry.h"
#include "util/windows/win-version.h"
#include "util/dstr.h"
#include "obs.h"
#include "obs-host/com.h"
#include "obs-host/display-clear-workaround.h"
#include "obs-host/graphics-thread-hooks.h"
#include "obs-host/module-symbols.h"
#include "obs-host/shared-texture.h"

#include <windows.h>
#include <wscapi.h>
#include <iwscapi.h>

/* util/windows/device-enum.c */
#include "util/windows/device-enum.h"

#include <dxgi.h>

/* util/windows/window-helpers.c */
#include "util/windows/window-helpers.h"

#include <util/windows/obfuscate.h>

#include <dwmapi.h>
#include <psapi.h>

/* audio-monitoring/win32/wasapi-enum-devices.c (MinGW defines the key itself) */
#ifndef __MINGW32__
#include <functiondiscoverykeys_devpkey.h>
#endif
