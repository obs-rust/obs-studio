/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-audio-input-path.
 * obs-source-audio-input-path.c reads and writes the audio input fields of struct
 * obs_source (obs-internal.h) and asks the input decisions
 * (obs-source-audio-input.h) and the resampler (media-io/audio-resampler.h), not
 * the rest of the core, so the C oracle can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/media-io/audio-resampler.h"
#include "libobs/obs-internal.h"
#include "libobs/obs-source-audio-input.h"
