/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-monitoring. obs-source-monitoring.c needs only its
 * declarations and constants (obs-source-monitoring.h, with stdbool.h and
 * stdint.h) and isnan() (math.h), not the core, so the C oracle and the C
 * test can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include <math.h>

#include "libobs/obs-source-monitoring.h"
