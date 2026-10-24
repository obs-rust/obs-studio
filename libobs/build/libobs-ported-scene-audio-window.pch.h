/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-scene-audio-window. obs-scene-audio-window.c needs only its
 * declarations (obs-scene-audio-window.h), AUDIO_OUTPUT_FRAMES and
 * ns_to_audio_frames() (media-io/audio-io.h, with util_mul_div64()), not the
 * core, so the C oracle and the C test can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/media-io/audio-io.h"
#include "libobs/obs-scene-audio-window.h"
