/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-async-upload. obs-source-async-upload.c needs its
 * declarations (obs-source-async-upload.h), the gs_color_format,
 * video_format and video_trc enums and OBS_SOURCE_FRAME_LINEAR_ALPHA (obs.h)
 * and OBS_SOURCE_CONVERT_NONE (obs-source-conversion-plan.h), so the C oracle
 * and the C test can compile it on their own. The implementation file
 * includes this header and nothing else. */

#pragma once

#include "obs.h"
#include "obs-source-async-upload.h"
#include "obs-source-conversion-plan.h"
