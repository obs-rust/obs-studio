/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-effect-param-types. graphics/effect-param-types.c needs only
 * its declarations (graphics/effect-param-types.h, with graphics.h for the
 * shader parameter types), strcmp() and astrcmp_n(), so the C oracle and the
 * C test can compile it on its own. The implementation file includes this
 * header and nothing else. */

#pragma once

#include <string.h>

#include "graphics/effect-param-types.h"
#include "util/dstr.h"
