/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-source-audio-render. obs-source-audio-render.c needs only
 * its declarations (obs-source-audio-render.h, with stdbool.h, stddef.h and
 * stdint.h), not the core, so the C oracle and the C test can compile it on
 * its own. The implementation file includes this header and nothing else. */

#pragma once

#include "libobs/obs-source-audio-render.h"
