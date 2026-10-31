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

#include "build/libobs-ported-effect-param-types.pch.h"

/* The parameter type decisions of effect-parser.c, declared in
 * effect-param-types.h. Ported to rust/obs-graphics (effect_param_types): the
 * hybrid libobs drops this file. */

enum gs_shader_param_type obs_effect_param_type(const char *type)
{
	if (strcmp(type, "float") == 0)
		return GS_SHADER_PARAM_FLOAT;
	else if (strcmp(type, "float2") == 0)
		return GS_SHADER_PARAM_VEC2;
	else if (strcmp(type, "float3") == 0)
		return GS_SHADER_PARAM_VEC3;
	else if (strcmp(type, "float4") == 0)
		return GS_SHADER_PARAM_VEC4;
	else if (strcmp(type, "int2") == 0)
		return GS_SHADER_PARAM_INT2;
	else if (strcmp(type, "int3") == 0)
		return GS_SHADER_PARAM_INT3;
	else if (strcmp(type, "int4") == 0)
		return GS_SHADER_PARAM_INT4;
	else if (astrcmp_n(type, "texture", 7) == 0)
		return GS_SHADER_PARAM_TEXTURE;
	else if (strcmp(type, "float4x4") == 0)
		return GS_SHADER_PARAM_MATRIX4X4;
	else if (strcmp(type, "bool") == 0)
		return GS_SHADER_PARAM_BOOL;
	else if (strcmp(type, "int") == 0)
		return GS_SHADER_PARAM_INT;
	else if (strcmp(type, "string") == 0)
		return GS_SHADER_PARAM_STRING;

	return GS_SHADER_PARAM_UNKNOWN;
}

enum obs_effect_param_route obs_effect_param_route(const char *type, bool is_texture)
{
	if (is_texture)
		return OBS_EFFECT_PARAM_ROUTE_TEXTURE;
	else if (strcmp(type, "int") == 0)
		return OBS_EFFECT_PARAM_ROUTE_INT;
	else if (strcmp(type, "float") == 0)
		return OBS_EFFECT_PARAM_ROUTE_FLOAT;
	else if (astrcmp_n(type, "int", 3) == 0)
		return OBS_EFFECT_PARAM_ROUTE_INT_ARRAY;
	else if (astrcmp_n(type, "float", 5) == 0)
		return OBS_EFFECT_PARAM_ROUTE_FLOAT_ARRAY;
	else if (astrcmp_n(type, "string", 6) == 0)
		return OBS_EFFECT_PARAM_ROUTE_STRING;
	else if (strcmp(type, "bool") == 0)
		return OBS_EFFECT_PARAM_ROUTE_BOOL;

	return OBS_EFFECT_PARAM_ROUTE_INVALID;
}

struct obs_effect_param_shape obs_effect_param_array_shape(const char *type, bool is_float)
{
	const char *intfloat_type = type + (is_float ? 5 : 3);
	struct obs_effect_param_shape shape = {0};

	if (intfloat_type[0] < '1' || intfloat_type[0] > '4')
		shape.bad_rows = true;

	shape.count = intfloat_type[0] - '0';

	if (intfloat_type[1] == 'x') {
		if (intfloat_type[2] < '1' || intfloat_type[2] > '4')
			shape.bad_cols = true;

		shape.count *= intfloat_type[2] - '0';
	}

	return shape;
}
