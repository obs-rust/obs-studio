/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.graphics-ffmpeg-rgba64: the 16-bit RGBA rows of graphics-ffmpeg.c.
 * graphics-ffmpeg-rgba64.c needs its declarations
 * (graphics/graphics-ffmpeg-rgba64.h, with stddef.h and stdint.h), string.h
 * for memcpy and srgb.h's conversions, not FFmpeg or the core, so the C oracle
 * and the C test can compile it on its own. The implementation file includes
 * this header and nothing else. */

#pragma once

#include <string.h>

#include "graphics/graphics-ffmpeg-rgba64.h"
#include "graphics/srgb.h"
