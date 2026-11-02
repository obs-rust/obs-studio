/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-encoder-audio-start-frames. obs-encoder-audio-start-frames.c
 * needs only its declarations (obs-encoder-audio-start-frames.h, with
 * stdint.h) and util_mul_div64() (a header inline), not the core, so the C
 * oracle and the C test can compile it on its own. The implementation file
 * includes this header and nothing else. */

#pragma once

#include "libobs/obs-encoder-audio-start-frames.h"
#include "libobs/util/util_uint64.h"
