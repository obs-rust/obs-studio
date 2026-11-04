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

/* The function parser of the effect parser (effect-parser.c) and the
 * lookups it shares with the rest. After a declaration's specifiers, type
 * and name, ep_parse_other() calls ep_parse_function() on the "(" token, with
 * the return type and name it allocated (ep_parse_function() owns them from
 * then on). It reads "( [in|inout|out|uniform] type name [: mapping], ... )",
 * an optional ": mapping" and the body from "{" to its matching "}" into the
 * function's contents, with the structs, functions, samplers and params the
 * signature and body name, then pushes the function into ep->funcs or frees
 * it.
 *
 * ep_check_for_keyword() steps past the current token when it is the keyword,
 * with a warning when *val was already set, and sets *val; it returns
 * PARSE_CONTINUE then, PARSE_SUCCESS when the token is another one, and
 * PARSE_EOF at the end of the text. ep_get_var_specifiers() calls it for a
 * declaration's specifiers.
 *
 * ep_getstruct() and ep_getsampler() find a struct or sampler by name, and
 * ep_getfunc_strref() and ep_getparam_strref() a function or param by a
 * token's text, or return NULL. The effect writer calls them too.
 *
 * Each walks ep->cfp's tokens from the one it is called on and adds its
 * errors to ep->cfp's error list; the caller goes on from the token where it
 * stopped.
 *
 * Private adapter, not a new public API: hidden in libobs, not EXPORT. */

#ifdef __cplusplus
extern "C" {
#endif

struct ep_struct *ep_getstruct(struct effect_parser *ep, const char *name);
struct ep_sampler *ep_getsampler(struct effect_parser *ep, const char *name);
struct ep_func *ep_getfunc_strref(struct effect_parser *ep, const struct strref *ref);
struct ep_param *ep_getparam_strref(struct effect_parser *ep, const struct strref *ref);
int ep_check_for_keyword(struct effect_parser *ep, const char *keyword, bool *val);
void ep_parse_function(struct effect_parser *ep, char *type, char *name);

#ifdef __cplusplus
}
#endif
