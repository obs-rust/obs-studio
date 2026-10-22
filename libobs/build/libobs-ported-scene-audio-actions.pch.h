/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-scene-audio-actions. obs-scene-audio-actions.c needs only
 * the action types and its two declarations (obs-scene-audio-actions.h,
 * with util/darray.h), AUDIO_OUTPUT_FRAMES and util_mul_div64(), not the
 * core, so the C oracle and the C tests can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/media-io/audio-io.h"
#include "libobs/obs-scene-audio-actions.h"
#include "libobs/util/util_uint64.h"
