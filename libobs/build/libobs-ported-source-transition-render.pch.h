/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-transition-render. obs-source-transition-render.c
 * needs only its declarations and constants
 * (obs-source-transition-render.h, with stdbool.h, stddef.h and stdint.h)
 * and util_mul_div64() (util/util_uint64.h), not the core, so the C oracle
 * and the C test can compile it on its own. The implementation file
 * includes this header and nothing else. */

#pragma once

#include "libobs/obs-source-transition-render.h"
#include "libobs/util/util_uint64.h"
