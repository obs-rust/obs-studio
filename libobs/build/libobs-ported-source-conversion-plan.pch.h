/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-conversion-plan. obs-source-conversion-plan.c needs
 * its declarations (obs-source-conversion-plan.h), the video_format,
 * video_trc and gs_color_format enums and the convert_video_format() inline
 * (obs-internal.h) and memset, so the C oracle and the C test can compile it
 * on their own. The implementation file includes this header and nothing
 * else. */

#pragma once

#include <string.h>

#include "obs-internal.h"
#include "obs-source-conversion-plan.h"
