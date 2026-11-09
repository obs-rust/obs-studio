/* Precompiled header (BUILD.md) for the libobs unity group libobs.effect:
 * the effect destroy. graphics/effect.c needs only the effect structs and
 * their header-inline frees (graphics/effect.h, which brings in the
 * graphics declarations and gs_shader_destroy()) and bfree(), not the rest
 * of the graphics module, so the C oracle can compile it on its own. The
 * implementation file includes this header and nothing else. */

#pragma once

#include "util/bmem.h"
#include "graphics/effect.h"
