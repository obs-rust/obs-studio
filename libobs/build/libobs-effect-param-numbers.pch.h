/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.effect-param-numbers: the effect parser's number reads.
 * graphics/effect-param-numbers.c needs only its declarations
 * (graphics/effect-param-types.h), strtol() and valid_int_str(), not the
 * parser, so the C oracle and the C test can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include <stdlib.h>

#include "graphics/effect-param-types.h"
#include "util/lexer.h"
