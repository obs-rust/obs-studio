/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.graphics-ffmpeg-route: the pixel route of graphics-ffmpeg.c.
 * graphics-ffmpeg-route.c needs its declarations
 * (graphics/graphics-ffmpeg-route.h, with stdbool.h, stddef.h and stdint.h)
 * and string.h for memcpy, not FFmpeg or the core, so the C oracle and the C
 * test can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include <string.h>

#include "graphics/graphics-ffmpeg-route.h"
