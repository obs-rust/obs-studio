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

/* The techniques of the effect parser (effect-parser.c): ep_parse() calls
 * ep_parse_technique() on the "technique" token. It reads the technique's
 * name and its "pass [name] { ... }" blocks, each with its
 * "vertex_shader = <call>;" and "pixel_shader = <call>;" commands (or
 * vertex_program and pixel_program), copies each call's tokens into the
 * pass's vertex_program or fragment_program, and pushes the technique into
 * ep->techniques.
 *
 * It walks ep->cfp's tokens from the keyword and adds its errors to
 * ep->cfp's error list; ep_parse() goes on from the token where it stopped.
 *
 * Private adapter, not a new public API: hidden in libobs, not EXPORT. */

#ifdef __cplusplus
extern "C" {
#endif

void ep_parse_technique(struct effect_parser *ep);

#ifdef __cplusplus
}
#endif
