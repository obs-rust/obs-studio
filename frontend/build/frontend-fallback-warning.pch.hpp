/* Precompiled header (BUILD.md) for the frontend unity groups
 * frontend.fallback-warning. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* utility/FallbackWarning.cpp */
#include "frontend/utility/FallbackWarning.hpp"
#include "frontend/dialogs/OBSFallbackWarning.hpp"

#ifdef OBS_FRONTEND_RUST
#include "frontend/utility/obs-fallback-warning.h"
#endif

#include <util/base.h>

#include <QApplication>
#include <QPointer>
#include <QTimer>
#include <QWidget>

#include <deque>
#include <memory>
#include <string>
