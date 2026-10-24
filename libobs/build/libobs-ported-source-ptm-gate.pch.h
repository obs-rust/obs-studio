/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-ptm-gate. obs-source-ptm-gate.c needs only its
 * declarations and constant (obs-source-ptm-gate.h, with stdbool.h and
 * stdint.h, which gives UINT64_MAX), not the core, so the C oracle and the C
 * test can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include "libobs/obs-source-ptm-gate.h"
