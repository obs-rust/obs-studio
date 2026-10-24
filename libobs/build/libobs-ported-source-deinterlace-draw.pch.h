/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-deinterlace-draw. obs-source-deinterlace-draw.c needs
 * its declarations (obs-source-deinterlace-draw.h), the obs_deinterlace_mode
 * and gs_color_space enums (obs.h) and TWOX_TOLERANCE
 * (obs-source-deinterlace-pacing.h), so the C oracle and the C test can
 * compile it on their own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include "obs.h"
#include "obs-source-deinterlace-draw.h"
#include "obs-source-deinterlace-pacing.h"
