/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-audio-mix. obs-audio-mix.c needs only its declarations and
 * types (obs-audio-mix.h, with media-io/audio-io.h and util/deque.h) and
 * util_mul_div64(), not the core, so the C oracle and the C tests can
 * compile it on its own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include "libobs/media-io/audio-io.h"
#include "libobs/obs-audio-mix.h"
#include "libobs/util/util_uint64.h"
