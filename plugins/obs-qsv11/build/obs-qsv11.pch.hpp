/* Precompiled header (BUILD.md) for the plugins/obs-qsv11 unity groups
 * obs-qsv11.core. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* QSV_Encoder.cpp */
#include "plugins/obs-qsv11/QSV_Encoder.h"
#include "plugins/obs-qsv11/QSV_Encoder_Internal.h"
#include "plugins/obs-qsv11/common_utils.h"
#include <obs-module.h>
#include <string>
#include <atomic>

/* QSV_Encoder_Internal.cpp */
#include <vpl/mfxstructures.h>
#include <vpl/mfxvideo++.h>
#include <vpl/mfxdispatcher.h>
#include <algorithm>
#include <chrono>

/* qsv-host interface (#860) */
#include "plugins/obs-qsv11/qsv-host.h"
