/* Precompiled header (BUILD.md) for the frontend unity groups
 * frontend.camera-busy. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* utility/CameraBusy.cpp */
#include "frontend/utility/CameraBusy.hpp"
#include "frontend/dialogs/OBSCameraBusy.hpp"

#ifdef OBS_FRONTEND_RUST
#include "frontend/utility/obs-camera-busy.h"
#endif

#include <util/base.h>
#include <util/platform.h>

#include <QApplication>
#include <QEvent>
#include <QPointer>
#include <QTimer>
#include <QWidget>

#include <atomic>
#include <deque>
#include <map>
#include <memory>
#include <string>
#include <thread>
