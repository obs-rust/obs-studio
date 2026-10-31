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

#include "build/libobs.pch.h"

/* The decisions of graphics.c's immediate-mode vertex builder, declared in
 * graphics-immediate.h. */

bool gs_immediate_unit_ok(bool immediate, int unit)
{
	(void)immediate;
	(void)unit;
	return true;
}

bool gs_immediate_push_ok(bool immediate, size_t num)
{
	return !(immediate && num == GS_IMMEDIATE_COUNT);
}

static inline size_t gs_immediate__min(const size_t a, const size_t b)
{
	return (a < b) ? a : b;
}

struct gs_immediate_draw gs_immediate_draw(size_t verts, size_t norms, size_t colors, size_t tex0)
{
	struct gs_immediate_draw draw = {0};

	draw.num = verts;
	if (!draw.num)
		return draw;

	if (norms && (norms != verts)) {
		draw.norms_mismatch = true;
		draw.num = gs_immediate__min(draw.num, norms);
	}

	if (colors && (colors != verts)) {
		draw.colors_mismatch = true;
		draw.num = gs_immediate__min(draw.num, colors);
	}

	if (tex0 && (tex0 != verts)) {
		draw.tex_mismatch = true;
		draw.num = gs_immediate__min(draw.num, tex0);
	}

	return draw;
}

struct gs_immediate_save gs_immediate_save(size_t verts, size_t norms, size_t colors,
					   const size_t tex[GS_IMMEDIATE_TEX_ARRAYS])
{
	struct gs_immediate_save save = {0};

	(void)norms;
	(void)colors;

	save.free_from = GS_IMMEDIATE_TEX_ARRAYS;
	save.num = verts;
	if (!save.num)
		return save;

	for (save.num_tex = 0; save.num_tex < GS_IMMEDIATE_TEX_ARRAYS; save.num_tex++)
		if (!tex[save.num_tex])
			break;

	return save;
}
