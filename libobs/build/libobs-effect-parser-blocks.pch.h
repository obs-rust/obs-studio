/* Precompiled header (BUILD.md) for the libobs unity group
 * libobs.effect-parser-blocks: the effect parser's struct and sampler-state
 * blocks. graphics/effect-parser-blocks.c needs only its declarations
 * (graphics/effect-parser-blocks.h, which brings in effect-parser.h and the
 * cf parser), not the rest of the graphics module. The implementation file
 * includes this header and nothing else. */

#pragma once

#include "graphics/effect-parser-blocks.h"
