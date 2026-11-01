/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-core-setup-rules. obs-core-setup-rules.c,
 * obs-core-setup-rules-ms.c and obs-core-setup-rules-task.c need only their
 * declarations and constants (obs-core-setup-rules.h, with stdbool.h,
 * stddef.h and stdint.h) and math.h for isfinite(), not the core, so the C
 * oracle and the C tests can compile them on their own. Each implementation
 * file includes this header and nothing else. */

#pragma once

#include <math.h>

#include "libobs/obs-core-setup-rules.h"
