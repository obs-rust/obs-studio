/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-transition-sample-time.
 * obs-source-transition-sample-time.c needs only its declaration
 * (obs-source-transition-sample-time.h, with stddef.h and stdint.h),
 * util_mul_div64() (util/util_uint64.h) and the clock it asks
 * (obs-source-transition-clock.h), not the core, so the C oracle and the C
 * test can compile it on their own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include "libobs/obs-source-transition-sample-time.h"
#include "libobs/obs-source-transition-clock.h"
#include "libobs/util/util_uint64.h"
