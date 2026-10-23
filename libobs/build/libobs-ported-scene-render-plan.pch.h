/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-scene-render-plan. obs-scene-render-plan.c.h needs its
 * declarations (obs-scene-render-plan.h, with graphics/matrix4.h),
 * close_float and EPSILON (graphics/math-defs.h), floorf and memcmp, all
 * without the core, so the C oracle and the C test can compile it on its
 * own. The implementation file includes this header and nothing else. */

#pragma once

#include <math.h>
#include <string.h>

#include "libobs/graphics/math-defs.h"
#include "libobs/obs-scene-render-plan.h"
