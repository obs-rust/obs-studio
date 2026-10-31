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

#include "build/libobs-ported-graphics-immediate.pch.h"

/* The decisions of graphics.c's immediate-mode vertex builder, declared in
 * graphics-immediate.h. Ported to rust/obs-graphics (immediate): the hybrid
 * libobs drops this file. */

size_t gs_immediate_cut(size_t num, size_t count)
{
	return (num < count) ? num : count;
}

bool gs_immediate_mismatch(size_t count, size_t verts)
{
	return count && (count != verts);
}

bool gs_immediate_push_ok(bool immediate, size_t num)
{
	return !(immediate && num == GS_IMMEDIATE_COUNT);
}

size_t gs_immediate_save_cut(size_t num, size_t count)
{
	(void)count;
	return num;
}

size_t gs_immediate_save_free_from(size_t num_tex)
{
	(void)num_tex;
	return GS_IMMEDIATE_TEX_ARRAYS;
}

bool gs_immediate_unit_ok(bool immediate, int unit)
{
	(void)immediate;
	(void)unit;
	return true;
}
