/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-video-copy-plan. obs-video-copy-plan.c needs only its
 * declarations (obs-video-copy-plan.h, with stdbool.h, stddef.h, stdint.h and
 * media-io/video-io.h for enum video_format) and labs() from stdlib.h, not
 * the core, so the C oracle and the C test can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include <stdlib.h>

#include "libobs/obs-video-copy-plan.h"
