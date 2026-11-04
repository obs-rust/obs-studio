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

/* The parameter declarations of the effect parser (effect-parser.c). After a
 * declaration's specifiers, type and name, ep_parse_other() calls
 * ep_parse_param() on the next token, with the type and name it allocated
 * (ep_parse_param() owns them from then on). It reads "[count]" with
 * ep_parse_param_array(), "< type name [= value]; ... >" into the param's
 * annotations, and "= value" with ep_parse_param_assign(), then wants the
 * ';', and pushes the param into ep->params or frees it.
 *
 * ep_parse_param_array() reads the count into param->array_count.
 * ep_parse_param_assign(), on the "=" token, reads the value (by
 * obs_effect_param_route(): a texture path or a string, an int, a float, a
 * bool, or the "{ ... }" and "type( ... )" forms of the vector and matrix
 * types) into param->default_val and steps to the next valid token; an
 * annotation entry calls it the same way. They stay declared here so the
 * value parser can be ported before the declaration that calls it.
 *
 * Each walks ep->cfp's tokens from the one it is called on and adds its
 * errors to ep->cfp's error list; the caller goes on from the token where it
 * stopped. The two helpers return false where the declaration is then
 * dropped.
 *
 * Private adapter, not a new public API: hidden in libobs, not EXPORT. */

#ifdef __cplusplus
extern "C" {
#endif

void ep_parse_param(struct effect_parser *ep, char *type, char *name, bool is_property, bool is_const, bool is_uniform);
bool ep_parse_param_array(struct effect_parser *ep, struct ep_param *param);
bool ep_parse_param_assign(struct effect_parser *ep, struct ep_param *param);

#ifdef __cplusplus
}
#endif
