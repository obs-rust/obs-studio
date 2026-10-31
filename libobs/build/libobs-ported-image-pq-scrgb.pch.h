/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-image-pq-scrgb: the ported JPEG XR PQ-to-scRGB conversion of
 * the image loader's Windows host. graphics-ffmpeg-host-pq.c needs its
 * declaration (graphics/graphics-ffmpeg-host-pq.h, with stdint.h), math.h for
 * powf and fabsf, string.h for memcpy and half.h's half_from_float(), not WIC,
 * windows.h or the core, so the C oracle and the C test can compile it on its
 * own. The implementation file includes this header and nothing else. */

#pragma once

#include <math.h>
#include <string.h>

#include "graphics/graphics-ffmpeg-host-pq.h"
#include "graphics/half.h"
