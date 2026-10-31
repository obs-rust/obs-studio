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

#include "graphics.h"

/* The parameter type decisions of the effect parser (effect-parser.c), on a
 * type name. One function per decision, each asked where the C made it:
 * - ep_compile_annotations() and ep_compile_param(): obs_effect_param_type(),
 *   the shader parameter type of a type name;
 * - ep_parse_param_assignment_val(): obs_effect_param_route(), the parser of
 *   a parameter's default value;
 * - ep_parse_param_assign_intfloat_array(): obs_effect_param_array_shape(),
 *   the number of values of an int or float array type, and whether its row
 *   or column digit is outside 1-4.
 *
 * effect-parser.c keeps every token call, every cf_adderror(), the value
 * parsers, the brace and constructor handling, the loop over the values and
 * the compile. Nothing here takes the parser or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* The default-value parser of a parameter, in the order
 * ep_parse_param_assignment_val() tests them. */
enum obs_effect_param_route {
	OBS_EFFECT_PARAM_ROUTE_TEXTURE,
	OBS_EFFECT_PARAM_ROUTE_INT,
	OBS_EFFECT_PARAM_ROUTE_FLOAT,
	OBS_EFFECT_PARAM_ROUTE_INT_ARRAY,
	OBS_EFFECT_PARAM_ROUTE_FLOAT_ARRAY,
	OBS_EFFECT_PARAM_ROUTE_STRING,
	OBS_EFFECT_PARAM_ROUTE_BOOL,
	OBS_EFFECT_PARAM_ROUTE_INVALID,
};

/* The values of an int or float array type: the row digit after the prefix,
 * times the column digit after an 'x'. Each digit outside 1-4 sets its flag
 * (effect-parser.c adds the error) and still counts, as a char minus '0'. */
struct obs_effect_param_shape {
	int count;
	bool bad_rows;
	bool bad_cols;
};

/* get_effect_param_type(): eleven exact names, any name starting with
 * "texture", and GS_SHADER_PARAM_UNKNOWN for anything else. */
enum gs_shader_param_type obs_effect_param_type(const char *type);

/* ep_parse_param_assignment_val(): the parser of a parameter of type, a
 * texture when is_texture. */
enum obs_effect_param_route obs_effect_param_route(const char *type, bool is_texture);

/* ep_parse_param_assign_intfloat_array(): the shape of type after its "float"
 * (is_float) or "int" prefix. type is longer than the prefix (the exact names
 * take the int and float routes); it reads the byte after the row digit, and
 * the one after that only after an 'x'. */
struct obs_effect_param_shape obs_effect_param_array_shape(const char *type, bool is_float);
