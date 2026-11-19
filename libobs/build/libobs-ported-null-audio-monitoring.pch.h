/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-null-audio-monitoring. null-audio-monitoring-reference.c needs
 * only its adapter declarations, NULL and UNUSED_PARAMETER, not the core, so
 * the C oracle and the C tests can compile it on its own. The implementation
 * file includes this header and nothing else. */

#pragma once

#include <stddef.h>

#include "libobs/audio-monitoring/null/null-audio-monitoring.h"
#include "libobs/util/c99defs.h"
