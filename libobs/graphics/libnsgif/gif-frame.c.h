/*
 * Copyright 2004 Richard Wilson <richard.wilson@netsurf-browser.org>
 * Copyright 2008 Sean Fox <dyntryx@gmail.com>
 * Copyright 2013-2022 Michael Drake <tlsa@netsurf-browser.org>
 *
 * This file is part of NetSurf's libnsgif, http://www.netsurf-browser.org/
 * Licenced under the MIT License,
 *                http://www.opensource.org/licenses/mit-license.php
 */

#include "build/libobs-ported-nsgif-frame.pch.h"

/* The frame pixel decode and disposal of libnsgif, declared in
 * gif-frame.h: the code of gif.c's nsgif__decode (with
 * nsgif__decode_simple, nsgif__decode_complex and their helpers),
 * nsgif__restore_bg and nsgif__recover_frame, on struct nsgif_canvas and
 * struct nsgif_frame_area in place of struct nsgif and struct nsgif_frame.
 * gif.c keeps those names as wrappers. The helpers have an nsgif_frame__
 * prefix, so they do not clash with gif.c's. Ported to rust/obs-graphics
 * (gif_frame): the hybrid libobs drops this file. */

/**
 * Convert an LZW result code to equivalent GIF result code.
 *
 * \param[in]  l_res  LZW response code.
 * \return GIF result code.
 */
static nsgif_error nsgif_frame__error_from_lzw(lzw_result l_res)
{
	static const nsgif_error g_res[] = {
		[LZW_OK]        = NSGIF_OK,
		[LZW_NO_MEM]    = NSGIF_ERR_OOM,
		[LZW_OK_EOD]    = NSGIF_ERR_END_OF_DATA,
		[LZW_NO_DATA]   = NSGIF_ERR_END_OF_DATA,
		[LZW_EOI_CODE]  = NSGIF_ERR_DATA_FRAME,
		[LZW_BAD_ICODE] = NSGIF_ERR_DATA_FRAME,
		[LZW_BAD_CODE]  = NSGIF_ERR_DATA_FRAME,
	};
	assert(l_res != LZW_BAD_PARAM);
	assert(l_res != LZW_NO_COLOUR);
	return g_res[l_res];
}

/**
 * Get the next line for GIF decode.
 *
 * Note that the step size must be initialised to 24 at the start of the frame
 * (when y == 0).  This is because of the first two passes of the frame have
 * the same step size of 8, and the step size is used to determine the current
 * pass.
 *
 * \param[in]     height     Frame height in pixels.
 * \param[in,out] y          Current row, starting from 0, updated on exit.
 * \param[in,out] step       Current step starting with 24, updated on exit.
 * \return true if there is a row to process, false at the end of the frame.
 */
static inline bool nsgif_frame__deinterlace(uint32_t height, uint32_t *y, uint8_t *step)
{
	*y += *step & 0xf;

	if (*y < height) return true;

	switch (*step) {
	case 24: *y = 4; *step = 8; if (*y < height) return true;
	         /* Fall through. */
	case  8: *y = 2; *step = 4; if (*y < height) return true;
	         /* Fall through. */
	case  4: *y = 1; *step = 2; if (*y < height) return true;
	         /* Fall through. */
	default:
		break;
	}

	return false;
}

/**
 * Get the next line for GIF decode.
 *
 * \param[in]     interlace  Non-zero if the frame is not interlaced.
 * \param[in]     height     Frame height in pixels.
 * \param[in,out] y          Current row, starting from 0, updated on exit.
 * \param[in,out] step       Current step starting with 24, updated on exit.
 * \return true if there is a row to process, false at the end of the frame.
 */
static inline bool nsgif_frame__next_row(uint32_t interlace,
		uint32_t height, uint32_t *y, uint8_t *step)
{
	if (!interlace) {
		return (++*y != height);
	} else {
		return nsgif_frame__deinterlace(height, y, step);
	}
}

/**
 * Get any frame clip adjustment for the image extent.
 *
 * \param[in]  frame_off  Frame's X or Y offset.
 * \param[in]  frame_dim  Frame width or height.
 * \param[in]  image_ext  Image width or height constraint.
 * \return the amount the frame needs to be clipped to fit the image in given
 *         dimension.
 */
static inline uint32_t nsgif_frame__clip(
		uint32_t frame_off,
		uint32_t frame_dim,
		uint32_t image_ext)
{
	uint32_t frame_ext = frame_off + frame_dim;

	if (frame_ext <= image_ext) {
		return 0;
	}

	return frame_ext - image_ext;
}

/**
 * Perform any jump over decoded data, to accommodate clipped portion of frame.
 *
 * \param[in,out] skip       Number of pixels of data to jump.
 * \param[in,out] available  Number of pixels of data currently available.
 * \param[in,out] pos        Position in decoded pixel value data.
 */
static inline void nsgif_frame__jump_data(
		uint32_t *skip,
		uint32_t *available,
		const uint8_t **pos)
{
	uint32_t jump = (*skip < *available) ? *skip : *available;

	*skip -= jump;
	*available -= jump;
	*pos += jump;
}

static nsgif_error nsgif_frame__decode_complex(
		struct lzw_ctx *lzw,
		const uint8_t *buf,
		size_t buf_len,
		const struct nsgif_canvas *canvas,
		uint32_t width,
		uint32_t height,
		uint32_t offset_x,
		uint32_t offset_y,
		uint32_t interlace,
		const uint8_t *data,
		uint32_t transparency_index,
		uint32_t *restrict frame_data,
		const uint32_t *restrict colour_table)
{
	lzw_result res;
	nsgif_error ret = NSGIF_OK;
	uint32_t clip_x = nsgif_frame__clip(offset_x, width, canvas->width);
	uint32_t clip_y = nsgif_frame__clip(offset_y, height, canvas->height);
	const uint8_t *uncompressed;
	uint32_t available = 0;
	uint8_t step = 24;
	uint32_t skip = 0;
	uint32_t y = 0;

	if (offset_x >= canvas->width ||
	    offset_y >= canvas->height) {
		return NSGIF_OK;
	}

	width -= clip_x;
	height -= clip_y;

	if (width == 0 || height == 0) {
		return NSGIF_OK;
	}

	/* Initialise the LZW decoding */
	res = lzw_decode_init(lzw, data[0],
			buf, buf_len,
			data + 1 - buf);
	if (res != LZW_OK) {
		return nsgif_frame__error_from_lzw(res);
	}

	do {
		uint32_t x;
		uint32_t *frame_scanline;

		frame_scanline = frame_data + offset_x +
				(y + offset_y) * canvas->rowspan;

		x = width;
		while (x > 0) {
			unsigned row_available;
			while (available == 0) {
				if (res != LZW_OK) {
					/* Unexpected end of frame, try to recover */
					if (res == LZW_OK_EOD ||
					    res == LZW_EOI_CODE) {
						ret = NSGIF_OK;
					} else {
						ret = nsgif_frame__error_from_lzw(res);
					}
					return ret;
				}
				res = lzw_decode(lzw,
						&uncompressed, &available);

				if (available == 0) {
					return NSGIF_OK;
				}
				nsgif_frame__jump_data(&skip, &available, &uncompressed);
			}

			row_available = x < available ? x : available;
			x -= row_available;
			available -= row_available;
			if (transparency_index > 0xFF) {
				while (row_available-- > 0) {
					*frame_scanline++ =
						colour_table[*uncompressed++];
				}
			} else {
				while (row_available-- > 0) {
					register uint32_t colour;
					colour = *uncompressed++;
					if (colour != transparency_index) {
						*frame_scanline =
							colour_table[colour];
					}
					frame_scanline++;
				}
			}
		}

		skip = clip_x;
		nsgif_frame__jump_data(&skip, &available, &uncompressed);
	} while (nsgif_frame__next_row(interlace, height, &y, &step));

	return ret;
}

static nsgif_error nsgif_frame__decode_simple(
		struct lzw_ctx *lzw,
		const uint8_t *buf,
		size_t buf_len,
		const struct nsgif_canvas *canvas,
		uint32_t height,
		uint32_t offset_y,
		const uint8_t *data,
		uint32_t transparency_index,
		uint32_t *restrict frame_data,
		const uint32_t *restrict colour_table)
{
	uint32_t pixels;
	uint32_t written = 0;
	nsgif_error ret = NSGIF_OK;
	lzw_result res;

	if (offset_y >= canvas->height) {
		return NSGIF_OK;
	}

	height -= nsgif_frame__clip(offset_y, height, canvas->height);

	if (height == 0) {
		return NSGIF_OK;
	}

	/* Initialise the LZW decoding */
	res = lzw_decode_init_map(lzw, data[0],
			transparency_index, colour_table,
			buf, buf_len,
			data + 1 - buf);
	if (res != LZW_OK) {
		return nsgif_frame__error_from_lzw(res);
	}

	frame_data += (offset_y * canvas->width);
	pixels = canvas->width * height;

	while (pixels > 0) {
		res = lzw_decode_map(lzw,
				frame_data, pixels, &written);
		pixels -= written;
		frame_data += written;
		if (res != LZW_OK) {
			/* Unexpected end of frame, try to recover */
			if (res == LZW_OK_EOD || res == LZW_EOI_CODE) {
				ret = NSGIF_OK;
			} else {
				ret = nsgif_frame__error_from_lzw(res);
			}
			break;
		}
	}

	if (pixels == 0) {
		ret = NSGIF_OK;
	}

	return ret;
}

/* exported function documented in gif-frame.h */
nsgif_error nsgif_frame_pixels_decode(
		struct lzw_ctx *lzw,
		const uint8_t *buf,
		size_t buf_len,
		size_t data_pos,
		const struct nsgif_canvas *canvas,
		const struct nsgif_frame_area *frame,
		const uint32_t *colour_table,
		bool data_complete)
{
	nsgif_error ret;
	const uint8_t *data = buf + data_pos;
	uint32_t width  = frame->rect.x1 - frame->rect.x0;
	uint32_t height = frame->rect.y1 - frame->rect.y0;
	uint32_t offset_x = frame->rect.x0;
	uint32_t offset_y = frame->rect.y0;
	uint32_t transparency_index = frame->transparency_index;
	uint32_t *restrict frame_data = canvas->bitmap;

	if (frame->interlaced == false && offset_x == 0 &&
			width == canvas->width &&
			width == canvas->rowspan) {
		ret = nsgif_frame__decode_simple(lzw, buf, buf_len, canvas,
				height, offset_y,
				data, transparency_index,
				frame_data, colour_table);
	} else {
		ret = nsgif_frame__decode_complex(lzw, buf, buf_len, canvas,
				width, height,
				offset_x, offset_y, frame->interlaced,
				data, transparency_index,
				frame_data, colour_table);
	}

	if (data_complete && ret == NSGIF_ERR_END_OF_DATA) {
		/* This is all the data there is, so make do. */
		ret = NSGIF_OK;
	}

	return ret;
}

/* exported function documented in gif-frame.h */
void nsgif_frame_restore_bg(
		const struct nsgif_canvas *canvas,
		const struct nsgif_frame_area *frame)
{
	enum {
		TRANSPARENT_COLOUR = 0x00,
	};
	uint32_t *bitmap = canvas->bitmap;
	size_t pixel_bytes = sizeof(*bitmap);

	if (frame == NULL) {
		size_t width  = canvas->width;
		size_t height = canvas->height;

		memset(bitmap, TRANSPARENT_COLOUR,
				width * height * pixel_bytes);
	} else {
		uint32_t width  = frame->rect.x1 - frame->rect.x0;
		uint32_t height = frame->rect.y1 - frame->rect.y0;
		uint32_t offset_x = frame->rect.x0;
		uint32_t offset_y = frame->rect.y0;

		if (frame->display == false ||
		    frame->rect.x0 >= canvas->width ||
		    frame->rect.y0 >= canvas->height) {
			return;
		}

		width -= nsgif_frame__clip(offset_x, width, canvas->width);
		height -= nsgif_frame__clip(offset_y, height, canvas->height);

		if (frame->transparency) {
			for (uint32_t y = 0; y < height; y++) {
				uint32_t *scanline = bitmap + offset_x +
						(offset_y + y) * canvas->width;
				memset(scanline, TRANSPARENT_COLOUR,
						width * pixel_bytes);
			}
		} else {
			for (uint32_t y = 0; y < height; y++) {
				uint32_t *scanline = bitmap + offset_x +
						(offset_y + y) * canvas->width;
				for (uint32_t x = 0; x < width; x++) {
					scanline[x] = canvas->background;
				}
			}
		}
	}
}

/* exported function documented in gif-frame.h */
void nsgif_frame_recover(
		const struct nsgif_canvas *canvas,
		const uint32_t *prev_frame)
{
	size_t pixel_bytes = sizeof(*canvas->bitmap);
	size_t height = canvas->height;
	size_t width  = canvas->width;

	memcpy(canvas->bitmap, prev_frame, height * width * pixel_bytes);
}
