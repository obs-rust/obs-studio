/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-video-frame-copy. obs-video-frame-copy.c needs only its
 * declarations (obs-video-frame-copy.h, with media-io/video-frame.h and
 * media-io/video-io.h for the frames and the output info), the copy plan it
 * runs (obs-video-copy-plan.h) and memcpy from string.h, not the core, so the
 * C oracle can compile it on its own. The implementation file includes this header and nothing else. */

#pragma once

#include <string.h>

#include "libobs/obs-video-copy-plan.h"
#include "libobs/obs-video-frame-copy.h"
