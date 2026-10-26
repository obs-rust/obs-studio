/*
 * Copyright 2004 Richard Wilson <richard.wilson@netsurf-browser.org>
 * Copyright 2008 Sean Fox <dyntryx@gmail.com>
 * Copyright 2013-2022 Michael Drake <tlsa@netsurf-browser.org>
 *
 * This file is part of NetSurf's libnsgif, http://www.netsurf-browser.org/
 * Licenced under the MIT License,
 *                http://www.opensource.org/licenses/mit-license.php
 */

#include "build/libobs-ported-nsgif-blocks.pch.h"

/* The GIF block parsers of libnsgif, declared in gif-blocks.h: the code of
 * gif.c's nsgif__parse_header, nsgif__parse_logical_screen_descriptor,
 * nsgif__parse_frame_extensions (with the graphic control and application
 * extensions), nsgif__parse_image_descriptor and nsgif__colour_table_extract,
 * on a byte position into the data in place of a pointer, and on plain
 * values in place of struct nsgif and struct nsgif_frame. gif.c keeps those
 * names as wrappers. Ported to rust/obs-graphics (gif_blocks): the hybrid
 * libobs drops this file. */

/**
 * Parse the Graphic Control Extension.
 *
 * \param[in,out] ext   The frame fields to update.
 * \param[in]     data  The data to decode.
 * \param[in]     len   Byte length of data.
 * \return NSGIF_ERR_END_OF_DATA if more data is needed,
 *         NSGIF_OK for success.
 */
static nsgif_error nsgif_blocks__graphic_control(
		struct nsgif_blocks_frame_ext *ext,
		const uint8_t *data,
		size_t len)
{
	enum {
		GIF_MASK_TRANSPARENCY = 0x01,
		GIF_MASK_DISPOSAL     = 0x1c,
	};

	/* 6-byte Graphic Control Extension is:
	 *
	 *  +0  CHAR    Graphic Control Label
	 *  +1  CHAR    Block Size
	 *  +2  CHAR    __Packed Fields__
	 *              3BITS   Reserved
	 *              3BITS   Disposal Method
	 *              1BIT    User Input Flag
	 *              1BIT    Transparent Color Flag
	 *  +3  SHORT   Delay Time
	 *  +5  CHAR    Transparent Color Index
	 */
	if (len < 6) {
		return NSGIF_ERR_END_OF_DATA;
	}

	ext->delay = data[3] | (data[4] << 8);

	if (data[2] & GIF_MASK_TRANSPARENCY) {
		ext->transparency = true;
		ext->transparency_index = data[5];
	}

	ext->disposal = ((data[2] & GIF_MASK_DISPOSAL) >> 2);
	/* I have encountered documentation and GIFs in the
	 * wild that use 0x04 to restore the previous frame,
	 * rather than the officially documented 0x03.  I
	 * believe some (older?)  software may even actually
	 * export this way.  We handle this as a type of
	 * "quirks" mode. */
	if (ext->disposal == NSGIF_DISPOSAL_RESTORE_QUIRK) {
		ext->disposal = NSGIF_DISPOSAL_RESTORE_PREV;
	}

	/* if we are clearing the background then we need to
	 * redraw enough to cover the previous frame too. */
	ext->redraw_required =
			ext->disposal == NSGIF_DISPOSAL_RESTORE_BG ||
			ext->disposal == NSGIF_DISPOSAL_RESTORE_PREV;

	return NSGIF_OK;
}

/**
 * Check an app ext identifier and authentication code for loop count extension.
 *
 * \param[in] data  The data to decode.
 * \param[in] len   Byte length of data.
 * \return true if extension is a loop count extension.
 */
static bool nsgif_blocks__app_ext_is_loop_count(
		const uint8_t *data,
		size_t len)
{
	enum {
		EXT_LOOP_COUNT_BLOCK_SIZE = 0x0b,
	};

	assert(len > 13);
	(void)(len);

	if (data[1] == EXT_LOOP_COUNT_BLOCK_SIZE) {
		if (strncmp((const char *)data + 2, "NETSCAPE2.0", 11) == 0 ||
		    strncmp((const char *)data + 2, "ANIMEXTS1.0", 11) == 0) {
			return true;
		}
	}

	return false;
}

/**
 * Parse the application extension
 *
 * \param[in,out] loop_max  The animation's loop count.
 * \param[in]     data      The data to decode.
 * \param[in]     len       Byte length of data.
 * \return NSGIF_ERR_END_OF_DATA if more data is needed,
 *         NSGIF_OK for success.
 */
static nsgif_error nsgif_blocks__application(
		int *loop_max,
		const uint8_t *data,
		size_t len)
{
	/* 14-byte+ Application Extension is:
	 *
	 *  +0    CHAR    Application Extension Label
	 *  +1    CHAR    Block Size
	 *  +2    8CHARS  Application Identifier
	 *  +10   3CHARS  Appl. Authentication Code
	 *  +13   1-256   Application Data (Data sub-blocks)
	 */
	if (len < 17) {
		return NSGIF_ERR_END_OF_DATA;
	}

	if (nsgif_blocks__app_ext_is_loop_count(data, len)) {
		enum {
			EXT_LOOP_COUNT_SUB_BLOCK_SIZE = 0x03,
			EXT_LOOP_COUNT_SUB_BLOCK_ID   = 0x01,
		};
		if ((data[13] == EXT_LOOP_COUNT_SUB_BLOCK_SIZE) &&
		    (data[14] == EXT_LOOP_COUNT_SUB_BLOCK_ID)) {
			*loop_max = data[15] | (data[16] << 8);

			/* The value in the source data means repeat N times
			 * after the first implied play. A value of zero has
			 * the special meaning of loop forever. (The only way
			 * to play the animation once is  not to have this
			 * extension at all. */
			if (*loop_max > 0) {
				(*loop_max)++;
			}
		}
	}

	return NSGIF_OK;
}

/* exported function documented in gif-blocks.h */
nsgif_error nsgif_blocks_extensions(
		const uint8_t *data,
		size_t len,
		size_t *pos,
		bool decode,
		struct nsgif_blocks_frame_ext *ext,
		int *loop_max)
{
	enum {
		GIF_EXT_INTRODUCER      = 0x21,
		GIF_EXT_GRAPHIC_CONTROL = 0xf9,
		GIF_EXT_COMMENT         = 0xfe,
		GIF_EXT_PLAIN_TEXT      = 0x01,
		GIF_EXT_APPLICATION     = 0xff,
		GIF_BLOCK_TERMINATOR    = 0x00,
	};
	size_t nsgif_data = *pos;
	/* gif.c's pointer difference, end minus position, as an int: negative
	 * once a skip has gone past the end. */
	int nsgif_bytes = (int)((ptrdiff_t)len - (ptrdiff_t)nsgif_data);

	/* Initialise the extensions */
	while (nsgif_bytes > 0 && data[nsgif_data] == GIF_EXT_INTRODUCER) {
		bool block_step = true;
		nsgif_error ret;

		nsgif_data++;
		nsgif_bytes--;

		if (nsgif_bytes == 0) {
			return NSGIF_ERR_END_OF_DATA;
		}

		/* Switch on extension label */
		switch (data[nsgif_data]) {
		case GIF_EXT_GRAPHIC_CONTROL:
			if (decode) {
				ret = nsgif_blocks__graphic_control(
						ext,
						data + nsgif_data,
						nsgif_bytes);
				if (ret != NSGIF_OK) {
					return ret;
				}
			}
			break;

		case GIF_EXT_APPLICATION:
			if (decode) {
				ret = nsgif_blocks__application(
						loop_max, data + nsgif_data, nsgif_bytes);
				if (ret != NSGIF_OK) {
					return ret;
				}
			}
			break;

		case GIF_EXT_COMMENT:
			/* Move the pointer to the first data sub-block Skip 1
			 * byte for the extension label. */
			++nsgif_data;
			block_step = false;
			break;

		default:
			break;
		}

		if (block_step) {
			/* Move the pointer to the first data sub-block Skip 2
			 * bytes for the extension label and size fields Skip
			 * the extension size itself
			 */
			if (nsgif_bytes < 2) {
				return NSGIF_ERR_END_OF_DATA;
			}
			nsgif_data += 2 + data[nsgif_data + 1];
		}

		/* Repeatedly skip blocks until we get a zero block or run out
		 * of data.  This data is ignored by this gif decoder. */
		while (nsgif_data < len && data[nsgif_data] != GIF_BLOCK_TERMINATOR) {
			nsgif_data += data[nsgif_data] + 1;
			if (nsgif_data >= len) {
				return NSGIF_ERR_END_OF_DATA;
			}
		}
		nsgif_data++;
		nsgif_bytes = (int)((ptrdiff_t)len - (ptrdiff_t)nsgif_data);
	}

	if (nsgif_data > len) {
		nsgif_data = len;
	}

	/* Set buffer position and return */
	*pos = nsgif_data;
	return NSGIF_OK;
}

/* exported function documented in gif-blocks.h */
nsgif_error nsgif_blocks_image_descriptor(
		const uint8_t *data,
		size_t len,
		size_t *pos,
		bool decode,
		bool first_frame,
		struct nsgif_blocks_image *out,
		uint32_t *width,
		uint32_t *height)
{
	const uint8_t *desc = data + *pos;
	size_t avail = len - *pos;
	enum {
		NSGIF_IMAGE_DESCRIPTOR_LEN = 10u,
		NSGIF_IMAGE_SEPARATOR      = 0x2Cu,
		NSGIF_MASK_INTERLACE       = 0x40u,
	};

	assert(out != NULL);

	if (avail < NSGIF_IMAGE_DESCRIPTOR_LEN) {
		return NSGIF_ERR_END_OF_DATA;
	}

	if (decode) {
		uint32_t x, y, w, h;

		if (desc[0] != NSGIF_IMAGE_SEPARATOR) {
			return NSGIF_ERR_DATA_FRAME;
		}

		x = desc[1] | (desc[2] << 8);
		y = desc[3] | (desc[4] << 8);
		w = desc[5] | (desc[6] << 8);
		h = desc[7] | (desc[8] << 8);
		out->flags = desc[9];

		out->rect.x0 = x;
		out->rect.y0 = y;
		out->rect.x1 = x + w;
		out->rect.y1 = y + h;

		out->interlaced = out->flags & NSGIF_MASK_INTERLACE;

		/* Allow first frame to grow image dimensions. */
		if (first_frame) {
			if (x + w > *width) {
				*width = x + w;
			}
			if (y + h > *height) {
				*height = y + h;
			}
		}
	}

	*pos += NSGIF_IMAGE_DESCRIPTOR_LEN;
	return NSGIF_OK;
}

/* exported function documented in gif-blocks.h */
void nsgif_blocks_colour_table_decode(
		uint32_t colour_table[NSGIF_MAX_COLOURS],
		const struct nsgif_colour_layout *layout,
		size_t colour_table_entries,
		const uint8_t *data)
{
	uint8_t *entry = (uint8_t *)colour_table;

	while (colour_table_entries--) {
		/* Gif colour map contents are r,g,b.
		 *
		 * We want to pack them bytewise into the colour table,
		 * according to the client colour layout.
		 */

		entry[layout->r] = *data++;
		entry[layout->g] = *data++;
		entry[layout->b] = *data++;
		entry[layout->a] = 0xff;

		entry += sizeof(uint32_t);
	}
}

/* exported function documented in gif-blocks.h */
nsgif_error nsgif_blocks_colour_table(
		uint32_t colour_table[NSGIF_MAX_COLOURS],
		const struct nsgif_colour_layout *layout,
		size_t colour_table_entries,
		const uint8_t *data,
		size_t data_len,
		size_t *used,
		bool decode)
{
	if (data_len < colour_table_entries * 3) {
		return NSGIF_ERR_END_OF_DATA;
	}

	if (decode) {
		nsgif_blocks_colour_table_decode(colour_table, layout,
				colour_table_entries, data);
	}

	*used = colour_table_entries * 3;
	return NSGIF_OK;
}

/* exported function documented in gif-blocks.h */
nsgif_error nsgif_blocks_header(
		const uint8_t *data,
		size_t len,
		size_t *pos,
		bool strict)
{
	const uint8_t *header = data + *pos;

	if (len - *pos < 6) {
		return NSGIF_ERR_END_OF_DATA;
	}

	if (strncmp((const char *) header, "GIF", 3) != 0) {
		return NSGIF_ERR_DATA;
	}
	header += 3;

	if (strict == true) {
		if ((strncmp((const char *) header, "87a", 3) != 0) &&
		    (strncmp((const char *) header, "89a", 3) != 0)) {
			return NSGIF_ERR_DATA;
		}
	}

	*pos += 6;
	return NSGIF_OK;
}

/* exported function documented in gif-blocks.h */
nsgif_error nsgif_blocks_screen(
		const uint8_t *data,
		size_t len,
		size_t *pos,
		struct nsgif_screen *out)
{
	enum {
		GIF_COLOUR_TABLE_MASK      = 0x80,
		GIF_COLOUR_TABLE_SIZE_MASK = 0x07,
	};
	const uint8_t *lsd = data + *pos;

	if (len - *pos < 7) {
		return NSGIF_ERR_END_OF_DATA;
	}

	out->width = lsd[0] | (lsd[1] << 8);
	out->height = lsd[2] | (lsd[3] << 8);
	out->global_palette = lsd[4] & GIF_COLOUR_TABLE_MASK;
	out->colour_table_size = 2 << (lsd[4] & GIF_COLOUR_TABLE_SIZE_MASK);
	out->bg_index = lsd[5];
	out->aspect_ratio = lsd[6];
	out->loop_max = 1;

	*pos += 7;
	return NSGIF_OK;
}

/* exported function documented in gif-blocks.h */
void nsgif_blocks_screen_size_quirk(uint32_t *width, uint32_t *height)
{
	/* Some broken GIFs report the size as the screen size they
	 * were created in. As such, we detect for the common cases and
	 * set the sizes as 0 if they are found which results in the
	 * GIF being the maximum size of the frames.
	 */
	if (((*width == 640) && (*height == 480)) ||
	    ((*width == 640) && (*height == 512)) ||
	    ((*width == 800) && (*height == 600)) ||
	    ((*width == 1024) && (*height == 768)) ||
	    ((*width == 1280) && (*height == 1024)) ||
	    ((*width == 1600) && (*height == 1200)) ||
	    ((*width == 0) || (*height == 0)) ||
	    ((*width > 2048) || (*height > 2048))) {
		*width = 1;
		*height = 1;
	}
}
