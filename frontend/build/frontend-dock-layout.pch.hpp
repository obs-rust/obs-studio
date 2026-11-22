/* Precompiled header (BUILD.md) for the frontend unity groups
 * frontend.dock-layout. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* utility/DockLayout.cpp */
#include "frontend/utility/DockLayout.hpp"

#ifdef OBS_FRONTEND_RUST
#include "frontend/utility/obs-dock-layout.h"
#endif

#include <util/base.h>
#include <util/c99defs.h>
#include <util/config-file.h>

#include <QChildEvent>
#include <QCryptographicHash>
#include <QDockWidget>
#include <QEvent>
#include <QMainWindow>
#include <QMetaObject>
#include <QMouseEvent>

#include <algorithm>
#include <string>
#include <utility>
#include <vector>
