/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-core-setup-rules. obs-core-setup-rules.c and
 * obs-core-setup-rules-ms.c need only their declarations and constants
 * (obs-core-setup-rules.h, with stdbool.h, stddef.h and stdint.h), not the
 * core, so the C oracle and the C tests can compile them on their own. Each implementation file includes this header
 * and nothing else. */

#pragma once

#include "libobs/obs-core-setup-rules.h"
