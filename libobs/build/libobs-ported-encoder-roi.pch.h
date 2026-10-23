/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-encoder-roi. obs-encoder-roi.c.h needs its declarations
 * (obs-encoder-roi.h), struct obs_encoder_roi and OBS_ENCODER_CAP_ROI
 * (obs-encoder.h, through the public obs.h), UINT32_MAX and isnan, but not
 * the core (obs-internal.h), so the C oracle can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include <math.h>
#include <stdint.h>

#include "obs.h"
#include "obs-encoder-roi.h"
