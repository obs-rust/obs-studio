/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-audio.
 * obs-audio.c, the audio tick, reads and changes the audio fields of struct
 * obs_core and struct obs_source (obs-internal.h) and asks the discard,
 * minimum timestamp, mix and render decisions (obs-audio-discard.h,
 * obs-audio-min-ts.h, obs-audio-mix.h, obs-audio-render.h), not the rest of
 * the core, so the C oracle and test_audio_buffering_subsample can compile it
 * on its own. The implementation file includes this header and nothing
 * else. */

#pragma once

#include <inttypes.h>
#include <string.h>
#include "libobs/obs-internal.h"
#include "libobs/obs-audio-discard.h"
#include "libobs/obs-audio-min-ts.h"
#include "libobs/obs-audio-mix.h"
#include "libobs/obs-audio-render.h"
