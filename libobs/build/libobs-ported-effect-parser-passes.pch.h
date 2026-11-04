/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-effect-parser-passes: the effect parser's passes and
 * techniques. graphics/effect-parser-passes.c needs only its declarations
 * (graphics/effect-parser-passes.h, which brings in effect-parser.h and the
 * cf parser), not the rest of the graphics module, so the C oracle can
 * compile it on its own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include "graphics/effect-parser-passes.h"
