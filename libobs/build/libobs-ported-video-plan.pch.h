/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-video-plan. obs-video-plan.c needs only the plan structs
 * and declarations (obs-internal.h, which brings the video_format,
 * video_colorspace, gs_color_format, gs_color_space and obs_scale_type
 * enums), so the C oracle and the C tests can compile it on their own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "obs-internal.h"
