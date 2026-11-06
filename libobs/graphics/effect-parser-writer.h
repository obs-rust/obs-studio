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

/* The shader writer of the effect parser (effect-parser.c). For each pass,
 * the compile step calls ep_makeshaderstring() on the pass's vertex or pixel
 * call, the tokens of "Name(args)". It appends to shader the declarations of
 * the params the call's tokens name, then, when Name is one of ep->funcs,
 * that function with every param, sampler, struct and function it depends
 * on written before it, and a main() that returns the call. Each uniform
 * param it declares is pushed, by a copy of its name, into used_params,
 * which the caller owns and frees. A call with no tokens, only whitespace
 * or NULL appends nothing; otherwise every written flag in ep is cleared
 * afterwards.
 *
 * Private adapter, not a new public API: hidden in libobs, not EXPORT. */

#ifdef __cplusplus
extern "C" {
#endif

typedef DARRAY(struct dstr) dstr_array_t;

void ep_makeshaderstring(struct effect_parser *ep, struct dstr *shader, cf_token_array_t *shader_call,
			 dstr_array_t *used_params);

#ifdef __cplusplus
}
#endif
