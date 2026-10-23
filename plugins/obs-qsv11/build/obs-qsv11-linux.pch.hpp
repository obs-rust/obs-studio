/* Precompiled header (BUILD.md) for the plugins/obs-qsv11 unity groups
 * obs-qsv11.linux. C++. Every implementation file in them includes this header and
 * nothing else: the includes and macro setup below are theirs, in group and
 * file order. */

#pragma once

/* common_utils_linux.cpp */
#include "plugins/obs-qsv11/common_utils.h"

#include <time.h>
#include <cpuid.h>

#include <va/va_drm.h>
#include <va/va_drmcommon.h>
#include <va/va_str.h>
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
#include <fcntl.h>
#include <string.h>
#include <dirent.h>

#include <obs.h>
#include <obs-encoder.h>
#include <obs-nix-platform.h>
#include <graphics/graphics.h>
#include <util/c99defs.h>
#include <util/dstr.h>
#include <util/bmem.h>

/* qsv-probe interface (#1863) */
#include "plugins/obs-qsv11/qsv-probe.h"

/* qsv-host interface (#860) */
#include "plugins/obs-qsv11/qsv-host.h"
