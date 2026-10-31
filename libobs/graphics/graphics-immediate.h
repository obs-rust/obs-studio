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
 * (graphics-immediate.c), on plain values. One function per decision, each
 * asked where the C made it, from counts the caller reads at that point:
 * - the push gates of gs_vertex3v(), gs_normal3v(), gs_color() and
 *   gs_texcoord2v() (validvertsize()): gs_immediate_unit_ok(), asked by
 *   gs_texcoord2v() before it indexes texverts[unit], then
 *   gs_immediate_push_ok() on the count of the array it pushes to;
 * - gs_render_stop(), for each of the normal, colour and texture 0 arrays:
 *   gs_immediate_mismatch() (it logs an error), then gs_immediate_cut() on
 *   the count read again after the log;
 * - gs_render_save(), after its texture array loop: gs_immediate_save_cut()
 *   on the normal, colour and every taken texture count, for the buffer's
 *   vertex count, and gs_immediate_save_free_from(), the texture arrays it
 *   leaves to free.
 *
 * graphics.c keeps gs_valid, every read of the arrays' counts, the no-vertex
 * tests and the texture array loop, every da_init, da_free and
 * da_push_back, gs_vbdata_create() and gs_vbdata_destroy(), the immediate
 * buffer's pointer and capacity stores and the memset of its colours, the
 * vertex buffer calls, gs_draw() and every blog(). Nothing here takes the
 * graphics context or keeps state.
 *
 * Hidden in libobs: not EXPORT. */

/* IMMEDIATE_COUNT in graphics.c: the vertices of the immediate buffer. */
#define GS_IMMEDIATE_COUNT 512
/* The texture arrays of the graphics context (texverts). */
#define GS_IMMEDIATE_TEX_ARRAYS 16

/* gs_render_stop(): the draw of num vertices, cut to a differing array of
 * count. */
size_t gs_immediate_cut(size_t num, size_t count);

/* gs_render_stop(): an array of count differs from the verts vertices (an
 * empty one never does). */
bool gs_immediate_mismatch(size_t count, size_t verts);

/* validvertsize(): an array holding num vertices takes one more; in
 * immediate mode none past GS_IMMEDIATE_COUNT. */
bool gs_immediate_push_ok(bool immediate, size_t num);

/* gs_render_save(): the buffer's vertex count of num, with an array of
 * count that the buffer takes. The C here keeps num; the Rust port cuts it
 * to a shorter non-empty array. */
size_t gs_immediate_save_cut(size_t num, size_t count);

/* gs_render_save(): the first texture array, up to the last, that the buffer
 * of num_tex texture arrays leaves and graphics.c frees;
 * GS_IMMEDIATE_TEX_ARRAYS frees none. The C here frees none; the Rust port
 * frees them from num_tex on. */
size_t gs_immediate_save_free_from(size_t num_tex);

/* gs_texcoord2v(): the texture unit can be pushed to, asked before
 * texverts[unit] is read. The C here takes any unit; the Rust port only the
 * GS_IMMEDIATE_TEX_ARRAYS units, and in immediate mode only unit 0. */
bool gs_immediate_unit_ok(bool immediate, int unit);
