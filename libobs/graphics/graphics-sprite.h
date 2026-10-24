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
#include <stdint.h>

#include "vec2.h"
#include "vec3.h"

/* The sprite and texture upload geometry of graphics.c, on plain values
 * (graphics-sprite.c):
 * - the four points and texture coordinates of a sprite: normalized (0 to
 *   1), in texels for a rectangle texture, and for a subregion of a texture;
 * - gs_draw_quadf(): the refusals (a texture that is not 2D; no texture and
 *   a size of 0), whether an axis takes the texture's size, and which static
 *   sprite buffer a flip draws;
 * - gs_draw_sprite_subregion(): the "whole texture" shortcut;
 * - gs_texture_set_image(): the copy plan (bytes per row, the flipped start
 *   row, one copy or row by row);
 * - gs_texture_create(): the power-of-two test and the mipmap rules.
 *
 * graphics.c keeps the device calls, the vertex buffers (it passes their
 * points and texture coordinate arrays), the matrix stack, gs_texture_map()
 * and gs_texture_unmap(), every memcpy and the log lines. The functions take
 * no lock, keep no state, do not call into libobs and write only through the
 * points and coordinate pointers of gs_sprite_build().
 *
 * Hidden in libobs: not EXPORT. */

/* GS_FLIP_U and GS_FLIP_V of graphics.h; graphics.c checks they are equal. */
#define GS_SPRITE_FLIP_U (1 << 0)
#define GS_SPRITE_FLIP_V (1 << 1)

/* GS_BUILD_MIPMAPS and GS_RENDER_TARGET of graphics.h; graphics.c checks
 * they are equal. */
#define GS_SPRITE_BUILD_MIPMAPS (1 << 0)
#define GS_SPRITE_RENDER_TARGET (1 << 2)

/* The texture coordinates of a sprite's edges: u from left to right and v
 * from top to bottom, before a flip swaps them. */
struct gs_sprite_uv {
	float start_u;
	float end_u;
	float start_v;
	float end_v;
};

/* Writes a sprite of cx by cy: points (0, 0), (cx, 0), (0, cy) and (cx, cy),
 * with z and the padding lane 0, and texture coordinates (start_u, start_v),
 * (end_u, start_v), (start_u, end_v) and (end_u, end_v), in that order, to
 * points[0..3] and uvs[0..3]. */
void gs_sprite_build(struct vec3 *points, struct vec2 *uvs, float cx, float cy, struct gs_sprite_uv uv);

/* Normalized coordinates, 0 to 1, flipped on u with GS_FLIP_U and on v with
 * GS_FLIP_V; other bits of flip are ignored. */
struct gs_sprite_uv gs_sprite_uv_norm(uint32_t flip);

/* Coordinates in texels for a rectangle texture of width by height, 0 to the
 * size, flipped on u with GS_FLIP_U and on v with GS_FLIP_V. */
struct gs_sprite_uv gs_sprite_uv_rect(uint32_t width, uint32_t height, uint32_t flip);

/* Normalized coordinates of the subregion (x, y, cx, cy) of a texture of
 * tex_cx by tex_cy: x / tex_cx to (x + cx) / tex_cx, and the same on v, all
 * in float, flipped on u with GS_FLIP_U and on v with GS_FLIP_V. A texture of
 * size 0 divides by 0 (inf or NaN). */
struct gs_sprite_uv gs_sprite_uv_subregion(uint32_t x, uint32_t y, uint32_t cx, uint32_t cy, uint32_t tex_cx,
					   uint32_t tex_cy, uint32_t flip);

/* gs_draw_sprite_subregion() draws the whole texture as a plain sprite when
 * the subregion starts at (0, 0) and has the texture's size. */
bool gs_sprite_subregion_is_whole(uint32_t x, uint32_t y, uint32_t cx, uint32_t cy, uint32_t tex_cx, uint32_t tex_cy);

/* Why gs_draw_quadf() draws nothing; it logs each. */
enum gs_sprite_quad_refusal {
	GS_SPRITE_QUAD_DRAW = 0,
	/* "A sprite must be a 2D texture" */
	GS_SPRITE_QUAD_NOT_2D = 1,
	/* "A sprite cannot be drawn without a width/height" */
	GS_SPRITE_QUAD_NO_SIZE = 2,
};

/* With a texture, it must be 2D (texture_2d is read only then); without one,
 * neither size may be 0 (-0.0 is 0, NaN is not). */
enum gs_sprite_quad_refusal gs_sprite_quad_refusal(bool has_texture, bool texture_2d, float width, float height);

/* Whether gs_draw_quadf() takes this axis from the texture: a size of 0 or
 * -0.0. graphics.c asks the texture only then. */
bool gs_sprite_quad_texture_size(float size);

/* Whether gs_draw_quadf() draws a texture that is not a rectangle texture
 * with the flipped static sprite buffer (normalized coordinates flipped on v)
 * rather than the plain one: any flip bit set. */
bool gs_sprite_quad_flipped_buffer(uint32_t flip);

/* How gs_texture_set_image() copies height rows of linesize bytes into the
 * mapped texture's rows of linesize_out bytes. */
enum gs_texture_upload_mode {
	/* nothing to copy: a flip with a height of 0 */
	GS_TEXTURE_UPLOAD_NONE = 0,
	/* row by row, the source from its last row up: start at src_offset,
	 * then linesize lower each row */
	GS_TEXTURE_UPLOAD_FLIPPED = 1,
	/* one copy of whole_size bytes: the line sizes are equal */
	GS_TEXTURE_UPLOAD_WHOLE = 2,
	/* row by row, top down: linesize higher each row */
	GS_TEXTURE_UPLOAD_ROWS = 3,
};

struct gs_texture_upload_plan {
	/* the smaller of the two line sizes: the bytes each row copies */
	size_t row_copy;
	/* GS_TEXTURE_UPLOAD_WHOLE: row_copy * height */
	size_t whole_size;
	/* GS_TEXTURE_UPLOAD_FLIPPED: (height - 1) * linesize, the last row */
	size_t src_offset;
	enum gs_texture_upload_mode mode;
};

/* The copy plan; size_t arithmetic, wrapping as in graphics.c. */
struct gs_texture_upload_plan gs_texture_upload_plan(uint32_t linesize, uint32_t linesize_out, size_t height,
						     bool flip);

/* At least 2 and a power of two. */
bool gs_texture_is_pow2(uint32_t size);

/* The warning gs_texture_create() logs when it turns mipmaps off. */
enum gs_texture_mip_warning {
	GS_TEXTURE_MIP_OK = 0,
	/* "Cannot use mipmaps with a non-power-of-two texture." */
	GS_TEXTURE_MIP_NOT_POW2 = 1,
	/* "Cannot use mipmaps with render targets." */
	GS_TEXTURE_MIP_RENDER_TARGET = 2,
};

struct gs_texture_mip_plan {
	/* the flags and levels gs_texture_create() passes to the device */
	uint32_t flags;
	uint32_t levels;
	enum gs_texture_mip_warning warning;
};

/* A texture uses mipmaps with GS_BUILD_MIPMAPS or levels other than 1. Then
 * a width or height that is not a power of two (gs_texture_is_pow2()), or
 * else GS_RENDER_TARGET, clears GS_BUILD_MIPMAPS and sets levels to 1. */
struct gs_texture_mip_plan gs_texture_mip_plan(uint32_t width, uint32_t height, uint32_t flags, uint32_t levels);
