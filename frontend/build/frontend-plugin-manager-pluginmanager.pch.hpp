/* Precompiled header (BUILD.md) for the frontend unity groups
 * frontend.plugin-manager-pluginmanager. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* plugin-manager/PluginManager.cpp */
#include "frontend/plugin-manager/PluginManager.hpp"
#include "frontend/plugin-manager/PluginManagerWindow.hpp"

#include <OBSApp.hpp>
#include <qt-wrappers.hpp>
#include <widgets/OBSBasic.hpp>

#ifdef OBS_FRONTEND_RUST
#include "frontend/utility/obs-exit-watchdog.h"
#endif

#include <QMessageBox>

#include <nlohmann/json.hpp>

#include <algorithm>
#include <fstream>
