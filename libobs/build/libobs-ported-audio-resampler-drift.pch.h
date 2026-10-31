/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-audio-resampler-drift: the libswresample resampler's decisions.
 * audio-resampler-drift.c needs its declarations (audio-resampler-drift.h,
 * with audio-io.h) and <math.h>, not FFmpeg, so the C oracle and the C test
 * can compile it on its own. The implementation file includes this header
 * and nothing else. */

#pragma once

#include "media-io/audio-resampler-drift.h"

#include <math.h>
#include <stddef.h>
