/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.ported-shader-parser-blocks: the shader parser's sampler-state and
 * struct blocks. graphics/shader-parser-blocks.c needs only its declarations
 * (graphics/shader-parser-blocks.h, which brings in shader-parser.h and the
 * cf parser), not the rest of the graphics module, so the C oracle can
 * compile it on its own. The implementation file includes this header and
 * nothing else. */

#pragma once

#include "graphics/shader-parser-blocks.h"
