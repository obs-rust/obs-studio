/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-video-scaler-plan: the swscale scaler's decisions.
 * video-scaler-plan.c needs its declarations (video-scaler-plan.h, with
 * video-io.h) and <stddef.h>, not FFmpeg, so the C oracle and the C test can
 * compile it on its own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include "media-io/video-scaler-plan.h"

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
