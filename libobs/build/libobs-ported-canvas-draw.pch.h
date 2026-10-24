/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-canvas-draw. obs-canvas-draw.c needs its declarations
 * (obs-canvas-draw.h, which brings obs-source-draw-technique.h) and the
 * gs_color_space enum (obs.h), so the C oracle and the C test can compile it
 * on their own. The implementation file includes this header and nothing
 * else. */

#pragma once

#include "obs.h"
#include "obs-canvas-draw.h"
