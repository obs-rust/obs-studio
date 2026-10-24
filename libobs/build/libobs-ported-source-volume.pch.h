/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-volume. obs-source-volume.c needs only its
 * declarations (obs-source-volume.h, with stdbool.h, stddef.h and stdint.h)
 * and close_float (graphics/math-defs.h, with math.h), not the core, so the
 * C oracle and the C test can compile it on its own. The implementation
 * file includes this header and nothing else. */

#pragma once

#include "libobs/graphics/math-defs.h"
#include "libobs/obs-source-volume.h"
