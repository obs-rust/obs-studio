/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-video-pacing. obs-video-pacing.c needs only its declarations
 * (obs-video-pacing.h, with stdbool.h, stddef.h and stdint.h) and memcpy()
 * (string.h), not the core, so the C oracle and the C test can compile it on
 * its own. The implementation file includes this header and nothing else. */

#pragma once

#include <string.h>

#include "libobs/obs-video-pacing.h"
