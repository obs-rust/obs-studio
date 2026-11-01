/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.audio-resampler-plan: the libswresample resampler's output plan.
 * audio-resampler-plan.c needs its declarations (audio-resampler-drift.h, with
 * audio-io.h), not FFmpeg, so the C oracle and the C test can compile it on
 * its own. The implementation file includes this header and nothing else. */

#pragma once

#include "media-io/audio-resampler-drift.h"
