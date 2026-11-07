/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.effect-parser-compile: the effect parser's compile step.
 * graphics/effect-parser-compile.c needs only its declarations
 * (graphics/effect-parser-compile.h, which brings in effect-parser.h, the cf
 * parser and the dstr helpers), the shader writer it calls
 * (graphics/effect-parser-writer.h), the param types
 * (graphics/effect-param-types.h), the effect and graphics declarations
 * (graphics/effect.h), blog(), assert(), UINT_MAX and the string functions,
 * not the rest of the graphics module. The implementation file includes this
 * header and nothing else. */

#pragma once

#include <assert.h>
#include <limits.h>
#include <string.h>
#include "util/base.h"
#include "graphics/effect-param-types.h"
#include "graphics/effect-parser-compile.h"
#include "graphics/effect-parser-writer.h"
#include "graphics/effect.h"
