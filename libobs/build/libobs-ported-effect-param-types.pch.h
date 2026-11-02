/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-effect-param-types. graphics/effect-param-types.c needs only
 * its declarations (graphics/effect-param-types.h, with graphics.h for the
 * shader parameter types), strcmp() and astrcmp_n();
 * graphics/effect-param-numbers.c the same declarations, strtol() and
 * valid_int_str(). So the C oracle and the C tests can compile each on its
 * own. Every implementation file includes this header and nothing else. */

#pragma once

#include <stdlib.h>
#include <string.h>

#include "graphics/effect-param-types.h"
#include "util/dstr.h"
#include "util/lexer.h"
