/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-output-begin. obs-output-begin.c needs its declarations
 * (obs-output-begin.h, with stdbool.h, stddef.h and stdint.h) and
 * util_mul_div64() (util/util_uint64.h, which needs stdint.h first), not the
 * core, so the C oracle and the C test can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/obs-output-begin.h"
#include "libobs/util/util_uint64.h"
