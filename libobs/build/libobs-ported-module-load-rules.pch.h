/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-module-load-rules. obs-module-load-rules.c and
 * obs-module-load-rules-skip.c need only their declarations and constants
 * (obs-module-load-rules.h, with stdbool.h, stddef.h and stdint.h), not the
 * core, so the C oracle and the C test can compile each on its own. Each
 * implementation file includes this header and nothing else. */

#pragma once

#include "libobs/obs-module-load-rules.h"
