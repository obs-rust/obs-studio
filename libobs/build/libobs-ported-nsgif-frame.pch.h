/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-nsgif-frame. graphics/libnsgif/gif-frame.c needs only its
 * declarations (graphics/libnsgif/gif-frame.h, with lzw.h and nsgif.h) and
 * the C headers below, not the rest of libnsgif, so the C oracle and the C
 * test can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include <assert.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "graphics/libnsgif/lzw.h"
#include "graphics/libnsgif/nsgif.h"
#include "graphics/libnsgif/gif-frame.h"
