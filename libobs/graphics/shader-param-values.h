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

#include <stdbool.h>
#include <stddef.h>

/* The number reads of the shader parser (shader-parser.c), on a number
 * token's text. One function per read, each asked where the C read it:
 * - sp_parse_param_array(), on the count of "name[count]":
 *   shader_param_array_count();
 * - sp_parse_param_assign_intfloat(), on an int or float value:
 *   shader_param_int_value() and shader_param_float_value().
 *
 * shader-parser.c keeps every token call, the CFTOKEN_NUM tests, the '-'
 * token, the value count of a floatN or floatNxM default, the push of each
 * value and every cf_adderror(). Nothing here takes the parser or keeps
 * state.
 *
 * Hidden in libobs: not EXPORT. */

#ifdef __cplusplus
extern "C" {
#endif

/* sp_parse_param_array(): false when the token's first len bytes are not an
 * int (valid_int_str()), and *count is left alone; otherwise *count is
 * strtol() of str cast to int. str is read on past the token by strtol(), to
 * the first byte that is not a digit. */
bool shader_param_array_count(const char *str, size_t len, int *count);

/* sp_parse_param_assign_intfloat(): an int value, str the number token's
 * text (read on as for shader_param_array_count()), negated when a '-'
 * token came before it, then cast to int. */
int shader_param_int_value(const char *str, bool negative);

/* sp_parse_param_assign_intfloat(): a float value, os_strtod() of str (at
 * most its first 63 bytes) cast to float, negated when a '-' token came
 * before it. */
float shader_param_float_value(const char *str, bool negative);

#ifdef __cplusplus
}
#endif
