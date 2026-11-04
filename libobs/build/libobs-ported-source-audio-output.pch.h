/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-audio-output.
 * obs-source-audio-output.c reads and writes the audio output fields of struct
 * obs_source (obs-internal.h) and asks the timing and placement decisions
 * (obs-source-audio-timing.h, obs-source-audio-place.h), not the rest of the
 * core, so the C oracle can compile it on its own. The implementation file
 * includes this header and nothing else. */

#pragma once

#include "libobs/obs-internal.h"
#include "libobs/obs-source-audio-place.h"
#include "libobs/obs-source-audio-timing.h"
