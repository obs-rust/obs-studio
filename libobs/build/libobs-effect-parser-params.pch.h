/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.effect-parser-params: the effect parser's parameter declarations,
 * annotations and value assignments. graphics/effect-parser-params.c needs
 * only its declarations (graphics/effect-parser-params.h, which brings in
 * effect-parser.h and the cf parser), the parameter type decisions it asks
 * and os_strtod(), not the rest of the graphics module. The implementation
 * file includes this header and nothing else. */

#pragma once

#include <string.h>
#include "util/platform.h"
#include "graphics/effect-param-types.h"
#include "graphics/effect-parser-params.h"
