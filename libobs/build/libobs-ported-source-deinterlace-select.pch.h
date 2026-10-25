/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-deinterlace-select. obs-source-deinterlace-select.c
 * needs its declarations (obs-source-deinterlace-select.h, with stdbool.h,
 * stddef.h and stdint.h) and the pacing it calls, FRAME_TOLERANCE among it
 * (obs-source-deinterlace-pacing.h), not the core, so the C oracle and the C
 * test can compile it on their own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include "libobs/obs-source-deinterlace-pacing.h"
#include "libobs/obs-source-deinterlace-select.h"
