/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.effect-parser-funcs: the effect parser's function parser and its
 * lookups. graphics/effect-parser-funcs.c needs only its declarations
 * (graphics/effect-parser-funcs.h, which brings in effect-parser.h, the cf
 * parser and the dstr helpers) and strcmp(), not the rest of the graphics
 * module. The implementation file includes this header and nothing else. */

#pragma once

#include <string.h>
#include "graphics/effect-parser-funcs.h"
