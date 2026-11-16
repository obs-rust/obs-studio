/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-effect-parser-entry: the effect parser's entry point.
 * graphics/effect-parser-entry.c needs only the effect parser's
 * declarations (graphics/effect-parser.h, which brings in the cf parser and
 * the graphics declarations), the block, technique, parameter, function and
 * compile steps it calls (graphics/effect-parser-blocks.h,
 * effect-parser-passes.h, effect-parser-params.h, effect-parser-funcs.h and
 * effect-parser-compile.h), blog() and strlen(), not the rest of the
 * graphics module, so the C oracle can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include <string.h>
#include "util/base.h"
#include "graphics/effect-parser-blocks.h"
#include "graphics/effect-parser-compile.h"
#include "graphics/effect-parser-funcs.h"
#include "graphics/effect-parser-params.h"
#include "graphics/effect-parser-passes.h"
#include "graphics/effect-parser.h"
