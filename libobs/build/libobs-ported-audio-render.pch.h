/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-audio-render. obs-audio-render.c needs only its declarations
 * (obs-audio-render.h), MAX_AUDIO_MIXES from media-io/audio-io.h and
 * close_float() from graphics/math-defs.h, not the core, so the C oracle and
 * the C tests can compile it on its own. The implementation file includes
 * this header and nothing else. */

#pragma once

#include "libobs/graphics/math-defs.h"
#include "libobs/media-io/audio-io.h"
#include "libobs/obs-audio-render.h"
