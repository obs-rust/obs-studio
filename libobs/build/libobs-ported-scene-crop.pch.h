/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-scene-crop. obs-scene-crop.c.h needs its declarations
 * (obs-scene-crop.h), INT_MAX, INT_MIN and isnan, all without the core, so
 * a C test and the oracle can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include <limits.h>
#include <math.h>
#include <stdint.h>

#include "libobs/obs-scene-crop.h"
