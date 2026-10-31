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

/* The decisions of the immediate-mode vertex builder of graphics.c
 * (graphics-immediate.c), on plain counts:
 * - the push gates of gs_vertex3v(), gs_normal3v(), gs_color() and
 *   gs_texcoord2v() (validvertsize()): gs_immediate_unit_ok(), asked by
 *   gs_texcoord2v() before it indexes texverts[unit], then
 *   gs_immediate_push_ok() on the count of the array it pushes to;
 * - gs_render_stop(): gs_immediate_draw(), the number of vertices to draw
 *   and which of the other arrays differ from the vertex count;
 * - gs_render_save(): gs_immediate_save(), the vertex count of the buffer,
 *   its texture arrays and the ones it leaves.
 *
 * graphics.c keeps gs_valid, every read of the arrays' counts (it passes
 * them here), every da_init, da_free and da_push_back, gs_vbdata_create()
 * and gs_vbdata_destroy(), the immediate buffer's pointer and capacity
 * stores and the memset of its colours, the vertex buffer calls, gs_draw()
 * and every blog(). Nothing here takes the graphics context or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* IMMEDIATE_COUNT in graphics.c: the vertices of the immediate buffer. */
#define GS_IMMEDIATE_COUNT 512
/* The texture arrays of the graphics context (texverts). */
#define GS_IMMEDIATE_TEX_ARRAYS 16

/* gs_immediate_draw(): what gs_render_stop() draws. */
struct gs_immediate_draw {
	/* The vertices to draw; 0 when there is no vertex. */
	size_t num;
	/* The normal, colour and texture 0 arrays are not empty and differ
	 * from the vertex count; each logs an error. */
	bool norms_mismatch;
	bool colors_mismatch;
	bool tex_mismatch;
};

/* gs_immediate_save(): the vertex buffer gs_render_save() makes. */
struct gs_immediate_save {
	/* The buffer's vertex count; 0 when there is no vertex, and no
	 * buffer. */
	size_t num;
	/* The texture arrays the buffer takes: the leading non-empty ones. */
	size_t num_tex;
	/* The first texture array the buffer leaves that graphics.c frees,
	 * up to the last; GS_IMMEDIATE_TEX_ARRAYS frees none. */
	size_t free_from;
};

/* gs_texcoord2v(): the texture unit can be pushed to, asked before
 * texverts[unit] is read. The C here takes any unit. */
bool gs_immediate_unit_ok(bool immediate, int unit);

/* validvertsize(): an array holding num vertices takes one more; in
 * immediate mode none past GS_IMMEDIATE_COUNT. */
bool gs_immediate_push_ok(bool immediate, size_t num);

/* gs_render_stop(): from the counts of the vertex, normal, colour and
 * texture 0 arrays, the vertices to draw, cut to each other non-empty
 * count that differs. */
struct gs_immediate_draw gs_immediate_draw(size_t verts, size_t norms, size_t colors, size_t tex0);

/* gs_render_save(): from the counts of the vertex, normal and colour arrays
 * and of the GS_IMMEDIATE_TEX_ARRAYS texture arrays, the buffer to make.
 * The C here gives it the vertex count and frees no texture array. */
struct gs_immediate_save gs_immediate_save(size_t verts, size_t norms, size_t colors,
					   const size_t tex[GS_IMMEDIATE_TEX_ARRAYS]);
