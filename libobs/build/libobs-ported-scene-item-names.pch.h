/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-scene-item-names. obs-scene-item-names.c.h needs only its
 * declarations (obs-scene-item-names.h, with stdbool.h): the loaders call the
 * compare they are given, not astrcmpi() itself. No core header, so the C
 * oracle and the C test can compile it on its own. The implementation file
 * includes this header and nothing else. */

#pragma once

#include "libobs/obs-scene-item-names.h"
