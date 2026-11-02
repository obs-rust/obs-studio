/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.shader-param-values: the shader parser's number reads.
 * graphics/shader-param-values.c needs only its declarations
 * (graphics/shader-param-values.h), strtol(), valid_int_str() and
 * os_strtod(), not the parser, so the C oracle and the C test can compile
 * it on its own. The implementation file includes this header and nothing
 * else. */

#pragma once

#include <stdlib.h>

#include "graphics/shader-param-values.h"
#include "util/lexer.h"
#include "util/platform.h"
