/* Precompiled header (BUILD.md) for the plugins/win-dshow unity groups
 * win-dshow.windows. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* dshow-plugin.cpp */
#include <obs-module.h>
#include <strsafe.h>
#include <strmif.h>
#ifdef VIRTUALCAM_AVAILABLE
#include "virtualcam-guid.h"
#endif

/* win-dshow-encoder.cpp */
#include <obs-avc.h>
#include <util/darray.h>
#include <util/dstr.hpp>
#ifdef OBS_LEGACY
#include "libdshowcapture/dshowcapture.hpp"
#else
#include <dshowcapture.hpp>
#endif

/* win-dshow.cpp */
#include <objbase.h>

#include <obs.hpp>
#include <util/platform.h>
#include <util/windows/WinHandle.hpp>
#include <util/threading.h>
#ifdef OBS_LEGACY
#include "libdshowcapture/dshowcapture.hpp"
#else
#include <dshowcapture.hpp>
#endif
#include "plugins/win-dshow/ffmpeg-decode.h"
#include "plugins/win-dshow/encode-dstr.hpp"
#include "plugins/win-dshow/dshow-rotation.h"

#include <algorithm>
#include <limits>
#include <set>
#include <string>
#include <vector>
