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

#include "libobs/build/libobs-ported-source-volume-mul.pch.h"

void obs_source_volume_mul_block(float *out, size_t count, float vol)
{
	register float *end = out + count;

	while (out < end)
		*(out++) *= vol;
}

void obs_source_volume_mul_data(float *out, const float *vol_data, size_t count)
{
	register float *end = out + count;
	register const float *vol = vol_data;

	while (out < end)
		*(out++) *= *(vol++);
}
