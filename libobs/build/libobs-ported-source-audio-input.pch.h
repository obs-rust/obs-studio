/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-audio-input. obs-source-audio-input.c needs only its
 * declarations and constants (obs-source-audio-input.h, with stdbool.h,
 * stddef.h and stdint.h) and <math.h> for sinf, sqrtf and M_PI, not the
 * core, so the C oracle and the C test can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

/* M_PI from MSVC's <math.h>, as cmake/pch/obs-c-std.h gives the core */
#ifndef _USE_MATH_DEFINES
#define _USE_MATH_DEFINES
#endif

#include <math.h>

#include "libobs/obs-source-audio-input.h"
