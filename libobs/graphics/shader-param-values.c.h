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

#include "build/libobs-ported-graphics.pch.h"

/* The number reads of shader-parser.c, declared in shader-param-values.h.
 * Ported to rust/obs-graphics (shader_param_values): the hybrid libobs drops
 * this file. */

bool shader_param_array_count(const char *str, size_t len, int *count)
{
	if (!valid_int_str(str, len))
		return false;

	*count = (int)strtol(str, NULL, 10);
	return true;
}

int shader_param_int_value(const char *str, bool negative)
{
	long l = strtol(str, NULL, 10);
	if (negative)
		l = -l;
	return (int)l;
}

float shader_param_float_value(const char *str, bool negative)
{
	float f = (float)os_strtod(str);
	if (negative)
		f = -f;
	return f;
}
