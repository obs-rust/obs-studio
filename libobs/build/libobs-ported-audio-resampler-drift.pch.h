/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-audio-resampler-drift: the libswresample resampler's decisions
 * and output plan. audio-resampler-drift.c and audio-resampler-plan.c need
 * their declarations (audio-resampler-drift.h, with audio-io.h) and <math.h>,
 * not FFmpeg, so the C oracle and the C tests can compile them on their own.
 * The implementation files include this header and nothing else. */

#pragma once

#include "media-io/audio-resampler-drift.h"

#include <math.h>
#include <stddef.h>
