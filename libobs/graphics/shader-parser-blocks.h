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

#include "graphics/shader-parser.h"

/* The sampler-state and struct blocks of the shader parser
 * (shader-parser.c), each called by shader_parse() on the keyword token
 * that starts it:
 * - sp_parse_sampler_state(), on "sampler_state": its name and its
 *   "state = value;" items, pushed into sp->samplers;
 * - sp_parse_struct(), on "struct": its name and its "type name;" or
 *   "type name : MAPPING;" variables, pushed into sp->structs.
 *
 * Each walks sp->cfp's tokens from the keyword and adds its errors to
 * sp->cfp's error list; shader_parse() goes on from the token where it
 * stopped.
 *
 * Hidden in libobs: not EXPORT. */

#ifdef __cplusplus
extern "C" {
#endif

void sp_parse_sampler_state(struct shader_parser *sp);
void sp_parse_struct(struct shader_parser *sp);

#ifdef __cplusplus
}
#endif
