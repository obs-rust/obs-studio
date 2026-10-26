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

#include "nsgif.h"

/* The GIF block parsers of libnsgif (gif-blocks.c), moved out of gif.c for
 * the port (#2488). They read the header, the logical screen descriptor,
 * the extensions before a frame, the image descriptor and the colour tables
 * from the GIF data, and return what they read in plain values: gif.c copies
 * it into struct nsgif and struct nsgif_frame, which stay in gif.c. Nothing
 * here allocates or keeps state.
 *
 * Every parser takes the whole GIF data (data, len) and a byte position pos
 * into it, and moves pos on success, as gif.c moved its pointer. pos may be
 * past len only where gif.c's pointer could be; the parsers read no byte at
 * or past len.
 *
 * nsgif_blocks_extensions() writes ext and loop_max as it parses, also when
 * it then returns an error (a later extension is short), as gif.c wrote the
 * frame and the gif: the caller copies them back in every case.
 *
 * Hidden in libobs: not EXPORT. */

/** Pixel format: colour component order. */
struct nsgif_colour_layout {
	uint8_t r; /**< Byte offset within pixel to red component. */
	uint8_t g; /**< Byte offset within pixel to green component. */
	uint8_t b; /**< Byte offset within pixel to blue component. */
	uint8_t a; /**< Byte offset within pixel to alpha component. */
};

/** What the logical screen descriptor sets (struct nsgif fields). */
struct nsgif_screen {
	uint32_t width;
	uint32_t height;
	bool global_palette;
	uint32_t colour_table_size;
	uint32_t bg_index;
	uint32_t aspect_ratio;
	/** The loop count before any loop count extension: 1. */
	int loop_max;
};

/** The frame fields that the graphic control extension sets. */
struct nsgif_blocks_frame_ext {
	uint32_t delay;
	uint8_t disposal;
	bool transparency;
	uint32_t transparency_index;
	bool redraw_required;
};

/** What the image descriptor sets on the frame. */
struct nsgif_blocks_image {
	uint32_t flags;
	nsgif_rect_t rect;
	bool interlaced;
};

/** The GIF header: "GIF" and, when strict, "87a" or "89a". 6 bytes. */
nsgif_error nsgif_blocks_header(const uint8_t *data, size_t len, size_t *pos, bool strict);

/** The logical screen descriptor. 7 bytes; out is set on NSGIF_OK only. */
nsgif_error nsgif_blocks_screen(const uint8_t *data, size_t len, size_t *pos, struct nsgif_screen *out);

/** The screen sizes that broken GIFs report (common screen sizes, 0, over
 * 2048) become 1x1, so the frames set the size. */
void nsgif_blocks_screen_size_quirk(uint32_t *width, uint32_t *height);

/** The extensions before a frame. With decode, the graphic control
 * extension sets ext and a loop count application extension sets
 * loop_max; every extension is skipped by its sub-blocks. */
nsgif_error nsgif_blocks_extensions(const uint8_t *data, size_t len, size_t *pos, bool decode,
				    struct nsgif_blocks_frame_ext *ext, int *loop_max);

/** The image descriptor. 10 bytes. With decode, out is set, and for the
 * first frame (first_frame) the frame can grow width and height. */
nsgif_error nsgif_blocks_image_descriptor(const uint8_t *data, size_t len, size_t *pos, bool decode,
					  bool first_frame, struct nsgif_blocks_image *out, uint32_t *width,
					  uint32_t *height);

/** Decode entries r,g,b triplets at data into colour_table in the layout
 * (alpha 0xff). No length check: nsgif_local_palette() decodes a table that
 * the scan already found complete. */
void nsgif_blocks_colour_table_decode(uint32_t colour_table[NSGIF_MAX_COLOURS],
				      const struct nsgif_colour_layout *layout, size_t entries,
				      const uint8_t *data);

/** A colour table of entries r,g,b triplets at data (data_len available),
 * decoded into colour_table in the layout when decode. *used is the byte
 * count on NSGIF_OK. */
nsgif_error nsgif_blocks_colour_table(uint32_t colour_table[NSGIF_MAX_COLOURS],
				      const struct nsgif_colour_layout *layout, size_t entries,
				      const uint8_t *data, size_t data_len, size_t *used, bool decode);
