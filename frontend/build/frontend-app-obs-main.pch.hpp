/* Precompiled header (BUILD.md) for the frontend unity groups
 * frontend.app-obs-main. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* obs-main.cpp */
#include <OBSApp.hpp>

#include <components/VolumeAccessibleInterface.hpp>
#include <utility/BaseLexer.hpp>
#include <utility/GpuStatus.hpp>
#include <utility/OBSTranslator.hpp>
#include <utility/platform.hpp>

#ifdef OBS_FRONTEND_RUST
#include "frontend/utility/obs-exit-watchdog.h"
#endif

#include <qt-wrappers.hpp>
#include <util/platform.h>
#include <util/profiler.h>
#include <util/util.hpp>

#include <QFontDatabase>
#include <QProcess>
#include <QPushButton>
#include <curl/curl.h>

#include <fstream>
#include <iostream>
#include <sstream>
#include <csignal>
#include <mutex>
#include <string>
#include <utility/obs-main-linux.hpp>
#include <utility/obs-main-macos.hpp>
#include <utility/obs-main-posix.hpp>
#include <utility/obs-main-windows.hpp>
