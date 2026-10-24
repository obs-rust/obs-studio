/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-module-type-rules. obs-module-type-rules.c needs only its
 * declarations and constants (obs-module-type-rules.h, with stdbool.h,
 * stddef.h and stdint.h) and strcmp() (string.h), not the core, so the C
 * oracle and the C test can compile it on its own. The implementation file
 * includes this header and nothing else. */

#pragma once

#include <string.h>

#include "libobs/obs-module-type-rules.h"
