/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.effect-parser-writer: the effect parser's shader writer.
 * graphics/effect-parser-writer.c needs only its declarations
 * (graphics/effect-parser-writer.h, which brings in effect-parser.h, the cf
 * parser and the dstr helpers), the lookups it shares with the function
 * parser (graphics/effect-parser-funcs.h) and strcmp(), not the rest of the
 * graphics module. The implementation file includes this header and nothing
 * else. */

#pragma once

#include <string.h>
#include "graphics/effect-parser-funcs.h"
#include "graphics/effect-parser-writer.h"
