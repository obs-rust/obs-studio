/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-audio-min-ts. obs-audio-min-ts.c needs only its declarations
 * (obs-audio-min-ts.h), media-io/audio-io.h and util_mul_div64(), not the
 * core, so the C oracle and the C tests can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/media-io/audio-io.h"
#include "libobs/obs-audio-min-ts.h"
#include "libobs/util/util_uint64.h"
