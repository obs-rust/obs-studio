/*
 * Copyright 2004 Richard Wilson <richard.wilson@netsurf-browser.org>
 * Copyright 2008 Sean Fox <dyntryx@gmail.com>
 * Copyright 2013-2022 Michael Drake <tlsa@netsurf-browser.org>
 *
 * This file is part of NetSurf's libnsgif, http://www.netsurf-browser.org/
 * Licenced under the MIT License,
 *                http://www.opensource.org/licenses/mit-license.php
 */

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#include "lzw.h"
#include "nsgif.h"

/* The frame pixel decode and disposal of libnsgif (gif-frame.c), moved out
 * of gif.c for the port (#2501). They write the pixels of one frame into
 * the client's bitmap, and clear or restore the bitmap before a frame.
 * gif.c builds the plain values below from struct nsgif and struct
 * nsgif_frame, which stay in gif.c, and keeps the bitmap callbacks, the
 * saved frame's allocation and the order of the steps.
 *
 * Hidden in libobs: not EXPORT. */

/** The client's bitmap and the image it holds (struct nsgif fields). */
struct nsgif_canvas {
	/** The pixels, rowspan * height (from the bitmap callbacks). */
	uint32_t *bitmap;
	/** The image size (gif->info.width, gif->info.height). */
	uint32_t width;
	uint32_t height;
	/** Pixels per row of the bitmap (gif->rowspan). */
	uint32_t rowspan;
	/** The background colour (gif->info.background). */
	uint32_t background;
};

/** The frame fields the decode and the restore read. */
struct nsgif_frame_area {
	nsgif_rect_t rect;
	bool interlaced;
	bool display;
	bool transparency;
	/** The transparent colour index; above 0xFF for none. */
	uint32_t transparency_index;
};

/** Decode one frame's image data into the canvas. data_pos is the
 * position of the frame's minimum code size in buf (buf_len bytes), and
 * must be inside it. colour_table has NSGIF_MAX_COLOURS entries. With
 * data_complete, data that ends inside the frame is a success. */
nsgif_error nsgif_frame_pixels_decode(struct lzw_ctx *lzw, const uint8_t *buf, size_t buf_len, size_t data_pos,
				      const struct nsgif_canvas *canvas, const struct nsgif_frame_area *frame,
				      const uint32_t *colour_table, bool data_complete);

/** Restore the canvas to the background before a frame: the whole image to
 * transparent (frame NULL), or the frame's rectangle, clipped to the image,
 * to transparent (frame->transparency) or to the background colour. */
void nsgif_frame_restore_bg(const struct nsgif_canvas *canvas, const struct nsgif_frame_area *frame);

/** Copy the saved frame (width * height pixels) back into the canvas. */
void nsgif_frame_recover(const struct nsgif_canvas *canvas, const uint32_t *prev_frame);
