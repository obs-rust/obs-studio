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

/* The struct and sampler-state blocks of the effect parser
 * (effect-parser.c), each called by ep_parse() on the keyword token that
 * starts it:
 * - ep_parse_struct(), on "struct": its name and its "type name;" or
 *   "type name : MAPPING;" variables, pushed into ep->structs;
 * - ep_parse_sampler_state(), on "sampler_state": its name and its
 *   "state = value;" items, pushed into ep->samplers.
 *
 * Each walks ep->cfp's tokens from the keyword and adds its errors to
 * ep->cfp's error list; ep_parse() goes on from the token where it
 * stopped.
 *
 * Private adapter, not a new public API: hidden in libobs, not EXPORT. */

#ifdef __cplusplus
extern "C" {
#endif

void ep_parse_struct(struct effect_parser *ep);
void ep_parse_sampler_state(struct effect_parser *ep);

#ifdef __cplusplus
}
#endif
