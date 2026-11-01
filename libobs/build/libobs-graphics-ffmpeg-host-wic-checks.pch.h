/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.graphics-ffmpeg-host-wic-checks: the file-name check and the
 * pixel-format pick of the image loader's Windows host.
 * graphics-ffmpeg-host-wic-checks.c needs their declarations
 * (graphics/graphics-ffmpeg-host-wic-checks.h, with stdbool.h and stddef.h)
 * and util/dstr.h's astrcmpi(), not WIC, windows.h or the core, so the C
 * oracle and the C test can compile it on its own. The implementation file
 * includes this header and nothing else. */

#pragma once

#include "graphics/graphics-ffmpeg-host-wic-checks.h"
#include "util/dstr.h"
