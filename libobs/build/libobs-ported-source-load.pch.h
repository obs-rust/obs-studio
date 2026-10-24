/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-load. obs-source-load.c needs only its declarations
 * (obs-source-load.h, with stdbool.h and stdint.h), FLT_MAX from float.h,
 * INT_MIN and INT_MAX from limits.h and INFINITY from math.h, not the core,
 * so the C oracle and the C test can compile it on their own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include <float.h>
#include <limits.h>
#include <math.h>

#include "libobs/obs-source-load.h"
