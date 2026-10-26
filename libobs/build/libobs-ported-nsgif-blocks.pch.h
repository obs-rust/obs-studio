/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-nsgif-blocks. graphics/libnsgif/gif-blocks.c needs only its
 * declarations (graphics/libnsgif/gif-blocks.h, with nsgif.h) and the C
 * headers below, not the rest of libnsgif, so the C oracle and the C test can
 * compile it on its own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include <assert.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#include "graphics/libnsgif/nsgif.h"
#include "graphics/libnsgif/gif-blocks.h"
