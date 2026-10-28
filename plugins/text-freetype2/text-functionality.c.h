/******************************************************************************
Copyright (C) 2014 by Nibbles

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

#include "plugins/text-freetype2/build/text-freetype2.pch.h"

float offsets[16] = {-2.0f, 0.0f, 0.0f, -2.0f, 2.0f,  0.0f, 2.0f,  0.0f,
		     0.0f,  2.0f, 0.0f, 2.0f,  -2.0f, 0.0f, -2.0f, 0.0f};

extern uint32_t texbuf_w, texbuf_h;

void draw_outlines(struct ft2_source *srcdata)
{
	if (!srcdata->text)
		return;

	gs_matrix_push();
	for (int32_t i = 0; i < 8; i++) {
		gs_matrix_translate3f(offsets[i * 2], offsets[(i * 2) + 1], 0.0f);
		draw_uv_vbuffer(srcdata->vbuf, srcdata->tex, srcdata->draw_effect, (uint32_t)wcslen(srcdata->text) * 6,
				false);
	}
	gs_matrix_identity();
	gs_matrix_pop();
}

void draw_drop_shadow(struct ft2_source *srcdata)
{
	if (!srcdata->text)
		return;

	gs_matrix_push();
	gs_matrix_translate3f(4.0f, 4.0f, 0.0f);
	draw_uv_vbuffer(srcdata->vbuf, srcdata->tex, srcdata->draw_effect, (uint32_t)wcslen(srcdata->text) * 6, false);
	gs_matrix_identity();
	gs_matrix_pop();
}

void set_up_vertex_buffer(struct ft2_source *srcdata)
{
	FT_UInt glyph_index = 0;
	uint32_t x = 0, space_pos = 0, word_width = 0;
	size_t len;

	if (!srcdata->text)
		return;

	if (srcdata->custom_width >= 100)
		srcdata->cx = srcdata->custom_width;
	else
		srcdata->cx = get_ft2_text_width(srcdata->text, srcdata);
	srcdata->cy = srcdata->max_h;

	obs_enter_graphics();
	if (srcdata->vbuf != NULL) {
		gs_vertbuffer_t *tmpvbuf = srcdata->vbuf;
		srcdata->vbuf = NULL;
		gs_vertexbuffer_destroy(tmpvbuf);
	}

	if (*srcdata->text == 0) {
		obs_leave_graphics();
		return;
	}

	srcdata->vbuf = create_uv_vbuffer((uint32_t)wcslen(srcdata->text) * 6, true);

	if (srcdata->custom_width <= 100)
		goto skip_word_wrap;
	if (!srcdata->word_wrap)
		goto skip_word_wrap;

	len = wcslen(srcdata->text);

	for (uint32_t i = 0; i <= len; i++) {
		if (i == wcslen(srcdata->text))
			goto eos_check;

		if (srcdata->text[i] != L' ' && srcdata->text[i] != L'\n')
			goto next_char;

	eos_check:;
		if (x + word_width > srcdata->custom_width) {
			if (space_pos != 0)
				srcdata->text[space_pos] = L'\n';
			x = 0;
		}
		if (i == wcslen(srcdata->text))
			goto eos_skip;

		x += word_width;
		word_width = 0;
		if (srcdata->text[i] == L'\n')
			x = 0;
		if (srcdata->text[i] == L' ')
			space_pos = i;
	next_char:;
		glyph_index = FT_Get_Char_Index(srcdata->font_face, (FT_ULong)srcdata->text[i]);
		if (src_glyph)
			word_width += (uint32_t)src_glyph->xadv;
	eos_skip:;
	}

skip_word_wrap:;
	fill_vertex_buffer(srcdata);
	gs_vertexbuffer_flush(srcdata->vbuf);
	obs_leave_graphics();
}

void fill_vertex_buffer(struct ft2_source *srcdata)
{
	struct gs_vb_data *vdata = gs_vertexbuffer_get_data(srcdata->vbuf);
	if (vdata == NULL || !srcdata->text)
		return;

	struct vec2 *tvarray = (struct vec2 *)vdata->tvarray[0].array;
	uint32_t *col = (uint32_t *)vdata->colors;

	FT_UInt glyph_index = 0;

	uint32_t dx = 0, dy = srcdata->max_h, max_y = dy;
	uint32_t cur_glyph = 0;
	uint32_t offset = 0;
	size_t len = wcslen(srcdata->text);

	if (srcdata->outline_text) {
		offset = 2;
		dx = offset;
	}

	for (size_t i = 0; i < len; i++) {
	add_linebreak:;
		if (srcdata->text[i] != L'\n')
			goto draw_glyph;
		dx = offset;
		i++;
		dy += srcdata->max_h + 4;
		if (i == wcslen(srcdata->text))
			goto skip_glyph;
		if (srcdata->text[i] == L'\n')
			goto add_linebreak;
	draw_glyph:;
		// Skip filthy dual byte Windows line breaks
		if (srcdata->text[i] == L'\r')
			goto skip_glyph;

		glyph_index = FT_Get_Char_Index(srcdata->font_face, (FT_ULong)srcdata->text[i]);
		if (src_glyph == NULL)
			goto skip_glyph;

		if (srcdata->custom_width < 100)
			goto skip_custom_width;

		if (dx + src_glyph->xadv > srcdata->custom_width) {
			dx = offset;
			dy += srcdata->max_h + 4;
		}

	skip_custom_width:;

		set_v3_rect(vdata->points + (cur_glyph * 6), (float)dx + (float)src_glyph->xoff,
			    (float)dy - (float)src_glyph->yoff, (float)src_glyph->w, (float)src_glyph->h);
		set_v2_uv(tvarray + (cur_glyph * 6), src_glyph->u, src_glyph->v, src_glyph->u2, src_glyph->v2);
		set_rect_colors2(col + (cur_glyph * 6), srcdata->color[0], srcdata->color[1]);
		dx += (uint32_t)src_glyph->xadv;
		const float glyph_bottom = (float)dy - (float)src_glyph->yoff + (float)src_glyph->h;
		if (glyph_bottom > (float)max_y)
			max_y = (uint32_t)glyph_bottom;
		cur_glyph++;
	skip_glyph:;
	}

	srcdata->cy = max_y;
}

void cache_standard_glyphs(struct ft2_source *srcdata)
{
	for (uint32_t i = 0; i < num_cache_slots; i++) {
		if (srcdata->cacheglyphs[i] != NULL) {
			bfree(srcdata->cacheglyphs[i]);
			srcdata->cacheglyphs[i] = NULL;
		}
	}

	srcdata->texbuf_x = 0;
	srcdata->texbuf_y = 0;

	cache_glyphs(srcdata, L"abcdefghijklmnopqrstuvwxyz"
			      L"ABCDEFGHIJKLMNOPQRSTUVWXYZ1234567890"
			      L"!@#$%^&*()-_=+,<.>/?\\|[]{}`~ \'\"\0");
}

FT_Render_Mode get_render_mode(struct ft2_source *srcdata);
FT_Render_Mode get_render_mode(struct ft2_source *srcdata)
{
	return srcdata->antialiasing ? FT_RENDER_MODE_NORMAL : FT_RENDER_MODE_MONO;
}

void load_glyph(struct ft2_source *srcdata, const FT_UInt glyph_index, const FT_Render_Mode render_mode);
void load_glyph(struct ft2_source *srcdata, const FT_UInt glyph_index, const FT_Render_Mode render_mode)
{
	const FT_Int32 load_mode = render_mode == FT_RENDER_MODE_MONO ? FT_LOAD_TARGET_MONO : FT_LOAD_DEFAULT;
	FT_Load_Glyph(srcdata->font_face, glyph_index, load_mode);
}

struct glyph_info *init_glyph(FT_GlyphSlot slot, const uint32_t dx, const uint32_t dy, const uint32_t g_w,
			      const uint32_t g_h);
struct glyph_info *init_glyph(FT_GlyphSlot slot, const uint32_t dx, const uint32_t dy, const uint32_t g_w,
			      const uint32_t g_h)
{
	struct glyph_info *glyph = bzalloc(sizeof(struct glyph_info));
	glyph->u = (float)dx / (float)texbuf_w;
	glyph->u2 = (float)(dx + g_w) / (float)texbuf_w;
	glyph->v = (float)dy / (float)texbuf_h;
	glyph->v2 = (float)(dy + g_h) / (float)texbuf_h;
	glyph->w = (int32_t)g_w;
	glyph->h = (int32_t)g_h;
	glyph->yoff = slot->bitmap_top;
	glyph->xoff = slot->bitmap_left;
	glyph->xadv = slot->advance.x >> 6;

	return glyph;
}

uint8_t get_pixel_value(const unsigned char *buf_row, FT_Render_Mode render_mode, const uint32_t x);
uint8_t get_pixel_value(const unsigned char *buf_row, FT_Render_Mode render_mode, const uint32_t x)
{
	if (render_mode == FT_RENDER_MODE_NORMAL) {
		return buf_row[x];
	}

	const uint32_t byte_index = x / 8;
	const uint8_t bit_index = x % 8;
	const bool pixel_set = (buf_row[byte_index] >> (7 - bit_index)) & 1;
	return pixel_set ? 255 : 0;
}

void rasterize(struct ft2_source *srcdata, FT_GlyphSlot slot, const FT_Render_Mode render_mode, const uint32_t dx,
	       const uint32_t dy);
void rasterize(struct ft2_source *srcdata, FT_GlyphSlot slot, const FT_Render_Mode render_mode, const uint32_t dx,
	       const uint32_t dy)
{
	/**
	 * The pitch's absolute value is the number of bytes taken by one bitmap
	 * row, including padding.
	 *
	 * Source: https://www.freetype.org/freetype2/docs/reference/ft2-basic_types.html
	 */
	const uint32_t pitch = (uint32_t)abs(slot->bitmap.pitch);

	for (uint32_t y = 0; y < slot->bitmap.rows; y++) {
		const uint32_t row_start = y * pitch;
		const uint32_t row = (dy + y) * texbuf_w;

		for (uint32_t x = 0; x < slot->bitmap.width; x++) {
			const uint32_t row_pixel_position = dx + x;
			const uint8_t pixel_value = get_pixel_value(&slot->bitmap.buffer[row_start], render_mode, x);
			srcdata->texbuf[row_pixel_position + row] = pixel_value;
		}
	}
}

struct glyph_cache_ctx {
	struct ft2_source *srcdata;
	FT_Render_Mode render_mode;
};

static bool glyph_cache_is_cached(void *data, wchar_t ch)
{
	struct glyph_cache_ctx *ctx = data;
	struct ft2_source *srcdata = ctx->srcdata;
	const FT_UInt glyph_index = FT_Get_Char_Index(srcdata->font_face, (FT_ULong)ch);

	return src_glyph != NULL;
}

static void glyph_cache_render(void *data, wchar_t ch, uint32_t *w, uint32_t *h)
{
	struct glyph_cache_ctx *ctx = data;
	struct ft2_source *srcdata = ctx->srcdata;
	FT_GlyphSlot slot = srcdata->font_face->glyph;
	const FT_UInt glyph_index = FT_Get_Char_Index(srcdata->font_face, (FT_ULong)ch);

	load_glyph(srcdata, glyph_index, ctx->render_mode);
	FT_Render_Glyph(slot, ctx->render_mode);

	*w = slot->bitmap.width;
	*h = slot->bitmap.rows;
}

static void glyph_cache_store(void *data, wchar_t ch, uint32_t x, uint32_t y)
{
	struct glyph_cache_ctx *ctx = data;
	struct ft2_source *srcdata = ctx->srcdata;
	FT_GlyphSlot slot = srcdata->font_face->glyph;
	const FT_UInt glyph_index = FT_Get_Char_Index(srcdata->font_face, (FT_ULong)ch);

	src_glyph = init_glyph(slot, x, y, slot->bitmap.width, slot->bitmap.rows);
	rasterize(srcdata, slot, ctx->render_mode, x, y);
}

static void glyph_cache_clear(void *data)
{
	struct glyph_cache_ctx *ctx = data;
	struct ft2_source *srcdata = ctx->srcdata;

	for (uint32_t i = 0; i < num_cache_slots; i++) {
		bfree(srcdata->cacheglyphs[i]);
		srcdata->cacheglyphs[i] = NULL;
	}

	memset(srcdata->texbuf, 0, (size_t)texbuf_w * (size_t)texbuf_h);
}

static const struct glyph_atlas_ops glyph_cache_ops = {
	.is_cached = glyph_cache_is_cached,
	.render = glyph_cache_render,
	.store = glyph_cache_store,
	.clear = glyph_cache_clear,
};

void cache_glyphs(struct ft2_source *srcdata, wchar_t *cache_glyphs)
{
	if (!srcdata->font_face || !cache_glyphs)
		return;

	struct glyph_cache_ctx ctx = {
		.srcdata = srcdata,
		.render_mode = get_render_mode(srcdata),
	};
	struct glyph_atlas atlas = {
		.width = texbuf_w,
		.height = texbuf_h,
		.x = srcdata->texbuf_x,
		.y = srcdata->texbuf_y,
		.max_h = srcdata->max_h,
	};

	const struct glyph_atlas_result result =
		glyph_atlas_cache(&atlas, cache_glyphs, wcslen(cache_glyphs), &glyph_cache_ops, &ctx);

	srcdata->texbuf_x = atlas.x;
	srcdata->texbuf_y = atlas.y;
	srcdata->max_h = atlas.max_h;

	if (result.out_of_space)
		blog(LOG_WARNING, "Out of space trying to render glyphs");

	if (result.stored > 0) {

		obs_enter_graphics();

		if (srcdata->tex != NULL) {
			gs_texture_t *tmp_texture = srcdata->tex;
			srcdata->tex = NULL;
			gs_texture_destroy(tmp_texture);
		}

		srcdata->tex = gs_texture_create(texbuf_w, texbuf_h, GS_A8, 1,
						 (const uint8_t **)(uintptr_t)&srcdata->texbuf, 0);

		obs_leave_graphics();
	}
}

static void set_text_from_file(struct ft2_source *srcdata, const char *filename, wchar_t *text)
{
	if (text == NULL) {
		if (!srcdata->file_load_failed) {
			blog(LOG_WARNING, "Failed to open file %s", filename);
			srcdata->file_load_failed = true;
		}
		return;
	}

	bfree(srcdata->text);
	srcdata->text = text;
}

void load_text_from_file(struct ft2_source *srcdata, const char *filename)
{
	set_text_from_file(srcdata, filename, ft2_read_text_file(filename));
}

void read_from_end(struct ft2_source *srcdata, const char *filename)
{
	set_text_from_file(srcdata, filename, ft2_read_text_file_end(filename, srcdata->log_lines));
}

uint32_t get_ft2_text_width(wchar_t *text, struct ft2_source *srcdata)
{
	if (!text) {
		return 0;
	}

	FT_GlyphSlot slot = srcdata->font_face->glyph;
	uint32_t w = 0, max_w = 0;
	const size_t len = wcslen(text);
	for (size_t i = 0; i < len; i++) {
		const FT_UInt glyph_index = FT_Get_Char_Index(srcdata->font_face, (FT_ULong)text[i]);

		if (text[i] == L'\n')
			w = 0;
		else {
			if (src_glyph) {
				// Use the cached values.
				w += (uint32_t)src_glyph->xadv;
			} else {
				load_glyph(srcdata, glyph_index, get_render_mode(srcdata));
				w += (uint32_t)(slot->advance.x >> 6);
			}
			if (w > max_w)
				max_w = w;
		}
	}

	return max_w;
}
