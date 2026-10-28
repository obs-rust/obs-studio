/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-graphics-ffmpeg-orient. graphics/graphics-ffmpeg-orient.c needs
 * only its declarations (graphics/graphics-ffmpeg-orient.h, with stdbool.h,
 * stddef.h and stdint.h) and string.h for memcpy, not FFmpeg or the core, so
 * the C oracle and the C test can compile it on its own. The implementation
 * file includes this header and nothing else. */

#pragma once

#include <string.h>

#include "libobs/graphics/graphics-ffmpeg-orient.h"
