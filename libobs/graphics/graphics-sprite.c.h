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

#include "libobs/build/libobs-ported-sprite.pch.h"

/* The sprite and texture upload geometry of graphics.c, moved from
 * assign_sprite_rect(), assign_sprite_uv(), build_sprite(),
 * build_sprite_norm(), build_subsprite_norm(), build_sprite_rect(),
 * gs_draw_quadf(), gs_draw_sprite_subregion(), gs_texture_set_image(),
 * is_pow2() and gs_texture_create(): the decisions and the float arithmetic,
 * on values. graphics.c keeps the device calls, the vertex buffers, the
 * matrix stack, the texture map and every memcpy, and the logs. Declared in
 * graphics-sprite.h without EXPORT. In hybrid libobs the Rust port
 * (rust/obs-graphics, sprite) replaces this file. */

static inline void sprite_assign_rect(float *start, float *end, float size, bool flip)
{
	if (!flip) {
		*start = 0.0f;
		*end = size;
	} else {
		*start = size;
		*end = 0.0f;
	}
}

static inline void sprite_assign_uv(float *start, float *end, bool flip)
{
	if (!flip) {
		*start = 0.0f;
		*end = 1.0f;
	} else {
		*start = 1.0f;
		*end = 0.0f;
	}
}

void gs_sprite_build(struct vec3 *points, struct vec2 *uvs, float cx, float cy, struct gs_sprite_uv uv)
{
	vec3_zero(points);
	vec3_set(points + 1, cx, 0.0f, 0.0f);
	vec3_set(points + 2, 0.0f, cy, 0.0f);
	vec3_set(points + 3, cx, cy, 0.0f);
	vec2_set(uvs, uv.start_u, uv.start_v);
	vec2_set(uvs + 1, uv.end_u, uv.start_v);
	vec2_set(uvs + 2, uv.start_u, uv.end_v);
	vec2_set(uvs + 3, uv.end_u, uv.end_v);
}

struct gs_sprite_uv gs_sprite_uv_norm(uint32_t flip)
{
	struct gs_sprite_uv uv;

	sprite_assign_uv(&uv.start_u, &uv.end_u, (flip & GS_SPRITE_FLIP_U) != 0);
	sprite_assign_uv(&uv.start_v, &uv.end_v, (flip & GS_SPRITE_FLIP_V) != 0);
	return uv;
}

struct gs_sprite_uv gs_sprite_uv_rect(uint32_t width, uint32_t height, uint32_t flip)
{
	struct gs_sprite_uv uv;
	float fwidth = (float)width;
	float fheight = (float)height;

	sprite_assign_rect(&uv.start_u, &uv.end_u, fwidth, (flip & GS_SPRITE_FLIP_U) != 0);
	sprite_assign_rect(&uv.start_v, &uv.end_v, fheight, (flip & GS_SPRITE_FLIP_V) != 0);
	return uv;
}

struct gs_sprite_uv gs_sprite_uv_subregion(uint32_t x, uint32_t y, uint32_t cx, uint32_t cy, uint32_t tex_cx,
					   uint32_t tex_cy, uint32_t flip)
{
	struct gs_sprite_uv uv;
	float fsub_x = (float)x;
	float fsub_y = (float)y;
	float fsub_cx = (float)cx;
	float fsub_cy = (float)cy;
	float fcx = (float)tex_cx;
	float fcy = (float)tex_cy;

	if ((flip & GS_SPRITE_FLIP_U) == 0) {
		uv.start_u = fsub_x / fcx;
		uv.end_u = (fsub_x + fsub_cx) / fcx;
	} else {
		uv.start_u = (fsub_x + fsub_cx) / fcx;
		uv.end_u = fsub_x / fcx;
	}

	if ((flip & GS_SPRITE_FLIP_V) == 0) {
		uv.start_v = fsub_y / fcy;
		uv.end_v = (fsub_y + fsub_cy) / fcy;
	} else {
		uv.start_v = (fsub_y + fsub_cy) / fcy;
		uv.end_v = fsub_y / fcy;
	}

	return uv;
}

bool gs_sprite_subregion_is_whole(uint32_t x, uint32_t y, uint32_t cx, uint32_t cy, uint32_t tex_cx, uint32_t tex_cy)
{
	return x == 0 && y == 0 && cx == tex_cx && cy == tex_cy;
}

enum gs_sprite_quad_refusal gs_sprite_quad_refusal(bool has_texture, bool texture_2d, float width, float height)
{
	if (has_texture) {
		if (!texture_2d)
			return GS_SPRITE_QUAD_NOT_2D;
	} else {
		if (width == 0.0f || height == 0.0f)
			return GS_SPRITE_QUAD_NO_SIZE;
	}

	return GS_SPRITE_QUAD_DRAW;
}

bool gs_sprite_quad_texture_size(float size)
{
	return size == 0.0f;
}

bool gs_sprite_quad_flipped_buffer(uint32_t flip)
{
	return flip != 0;
}

struct gs_texture_upload_plan gs_texture_upload_plan(uint32_t linesize, uint32_t linesize_out, size_t height, bool flip)
{
	struct gs_texture_upload_plan plan = {0};

	plan.row_copy = (linesize < linesize_out) ? linesize : linesize_out;

	if (flip) {
		/* graphics.c computed (height - 1) * linesize before its loop
		 * found no row: with a height of 0 that wraps, and the source
		 * pointer it moved by it was undefined. No row was copied, so
		 * return that first. */
		if (height == 0) {
			plan.mode = GS_TEXTURE_UPLOAD_NONE;
			return plan;
		}
		plan.src_offset = (height - 1) * linesize;
		plan.mode = GS_TEXTURE_UPLOAD_FLIPPED;

	} else if (linesize == linesize_out) {
		plan.whole_size = plan.row_copy * height;
		plan.mode = GS_TEXTURE_UPLOAD_WHOLE;

	} else {
		plan.mode = GS_TEXTURE_UPLOAD_ROWS;
	}

	return plan;
}

bool gs_texture_is_pow2(uint32_t size)
{
	return size >= 2 && (size & (size - 1)) == 0;
}

struct gs_texture_mip_plan gs_texture_mip_plan(uint32_t width, uint32_t height, uint32_t flags, uint32_t levels)
{
	struct gs_texture_mip_plan plan;
	bool pow2tex = gs_texture_is_pow2(width) && gs_texture_is_pow2(height);
	bool uses_mipmaps = (flags & GS_SPRITE_BUILD_MIPMAPS || levels != 1);

	plan.warning = GS_TEXTURE_MIP_OK;

	if (uses_mipmaps && !pow2tex) {
		plan.warning = GS_TEXTURE_MIP_NOT_POW2;

		uses_mipmaps = false;
		flags &= ~GS_SPRITE_BUILD_MIPMAPS;
		levels = 1;
	}

	if (uses_mipmaps && flags & GS_SPRITE_RENDER_TARGET) {
		plan.warning = GS_TEXTURE_MIP_RENDER_TARGET;
		flags &= ~GS_SPRITE_BUILD_MIPMAPS;
		levels = 1;
	}

	plan.flags = flags;
	plan.levels = levels;
	return plan;
}
