/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-filter-chain. obs-source-filter-chain.c.h needs only
 * its declarations and constants (obs-source-filter-chain.h, with stdbool.h,
 * stddef.h and stdint.h), strcmp() and UNUSED_PARAMETER(), not the core, so
 * the C oracle and the C test can compile it on its own. The implementation
 * file includes this header and nothing else. */

#pragma once

#include <string.h>

#include "libobs/util/c99defs.h"
#include "libobs/obs-source-filter-chain.h"
