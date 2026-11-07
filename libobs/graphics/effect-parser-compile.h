/******************************************************************************
    Copyright (C) 2023 by Lain Bailey <lain@obsproject.com>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#pragma once

#include "graphics/effect-parser.h"

/* The compile step of the effect parser (effect-parser.c). Once ep_parse()
 * has read the whole text with no error, it calls ep_compile(), which fills
 * ep->effect from the parser: one gs_effect_param for each of ep->params
 * (with its annotations, its type and the parser's default value, moved out
 * of the parser; ViewProj and World are kept as the effect's view_proj and
 * world), and one gs_effect_technique for each of ep->techniques, with each
 * pass's vertex and pixel shader created from the text the shader writer
 * builds for its call, and paired with the effect's and the shader's param
 * for each uniform that text declares. A shader's compile errors go into
 * ep->cfp's error list. It returns false when a pass is missing a shader or
 * a shader param; whatever it created stays in the effect for effect_free().
 *
 * debug_param() and debug_print_string() are the parser's debug printers,
 * built only with _DEBUG and _DEBUG_SHADERS; ep_parse() and the compile step
 * both print with them.
 *
 * Private adapter, not a new public API: hidden in libobs, not EXPORT. */

#ifdef __cplusplus
extern "C" {
#endif

bool ep_compile(struct effect_parser *ep);

#if defined(_DEBUG) && defined(_DEBUG_SHADERS)
void debug_param(struct gs_effect_param *param, struct ep_param *param_in, unsigned long long idx, const char *offset);
void debug_print_string(const char *offset, const char *str);
#endif

#ifdef __cplusplus
}
#endif
