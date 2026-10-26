/*
 * Copyright 2004 Richard Wilson <richard.wilson@netsurf-browser.org>
 * Copyright 2008 Sean Fox <dyntryx@gmail.com>
 * Copyright 2013-2022 Michael Drake <tlsa@netsurf-browser.org>
 *
 * This file is part of NetSurf's libnsgif, http://www.netsurf-browser.org/
 * Licenced under the MIT License,
 *                http://www.opensource.org/licenses/mit-license.php
 */

#include "build/libobs-nsgif.pch.h"

/** Default minimum allowable frame delay in cs. */
#define NSGIF_FRAME_DELAY_MIN 2

/**
 * Default frame delay to apply.
 *
 * Used when a frame delay lower than the minimum is requested.
 */
#define NSGIF_FRAME_DELAY_DEFAULT 10

/** GIF frame data */
typedef struct nsgif_frame {
	struct nsgif_frame_info info;

	/** offset (in bytes) to the GIF frame data */
	size_t frame_offset;
	/** whether the frame has previously been decoded. */
	bool decoded;
	/** whether the frame is totally opaque */
	bool opaque;
	/** whether a full image redraw is required */
	bool redraw_required;

	/** Amount of LZW data found in scan */
	uint32_t lzw_data_length;

	/** the index designating a transparent pixel */
	uint32_t transparency_index;

	/** offset to frame colour table */
	uint32_t colour_table_offset;

	/* Frame flags */
	uint32_t flags;
} nsgif_frame;

/** GIF animation data */
struct nsgif {
	struct nsgif_info info;

	/** LZW decode context */
	void *lzw_ctx;
	/** callbacks for bitmap functions */
	nsgif_bitmap_cb_vt bitmap;
	/** decoded frames */
	nsgif_frame *frames;
	/** current frame */
	uint32_t frame;
	/** current frame decoded to bitmap */
	uint32_t decoded_frame;

	/** currently decoded image; stored as bitmap from bitmap_create callback */
	nsgif_bitmap_t *frame_image;
	/** Row span of frame_image in pixels. */
	uint32_t rowspan;

	/** Minimum allowable frame delay. */
	uint16_t delay_min;

	/** Frame delay to apply when delay is less than \ref delay_min. */
	uint16_t delay_default;

	/** number of animation loops so far */
	int loop_count;

	/** number of frames partially decoded */
	uint32_t frame_count_partial;

	/**
	 * Whether all the GIF data has been supplied, or if there may be
	 * more to come.
	 */
	bool data_complete;

	/** pointer to GIF data */
	const uint8_t *buf;
	/** current index into GIF data */
	size_t buf_pos;
	/** total number of bytes of GIF data available */
	size_t buf_len;

	/** current number of frame holders */
	uint32_t frame_holders;
	/** background index */
	uint32_t bg_index;
	/** image aspect ratio (ignored) */
	uint32_t aspect_ratio;
	/** size of global colour table (in entries) */
	uint32_t colour_table_size;

	/** current colour table */
	uint32_t *colour_table;
	/** Client's colour component order. */
	struct nsgif_colour_layout colour_layout;
	/** global colour table */
	uint32_t global_colour_table[NSGIF_MAX_COLOURS];
	/** local colour table */
	uint32_t local_colour_table[NSGIF_MAX_COLOURS];

	/** previous frame for NSGIF_FRAME_RESTORE */
	void *prev_frame;
	/** previous frame index */
	uint32_t prev_index;
};

/**
 * Helper macro to get number of elements in an array.
 *
 * \param[in]  _a  Array to count elements of.
 * \return NUlber of elements in array.
 */
#define NSGIF_ARRAY_LEN(_a) ((sizeof(_a)) / (sizeof(*_a)))

/**
 *
 * \file
 * \brief GIF image decoder
 *
 * The GIF format is thoroughly documented; a full description can be found at
 * http://www.w3.org/Graphics/GIF/spec-gif89a.txt
 *
 * \todo Plain text and comment extensions should be implemented.
 */

/** Internal flag that the colour table needs to be processed */
#define NSGIF_PROCESS_COLOURS 0xaa000000

/** Internal flag that a frame is invalid/unprocessed */
#define NSGIF_FRAME_INVALID UINT32_MAX

/** No transparency */
#define NSGIF_NO_TRANSPARENCY (0xFFFFFFFFu)

/* GIF Flags */
#define NSGIF_COLOUR_TABLE_MASK 0x80
#define NSGIF_COLOUR_TABLE_SIZE_MASK 0x07
#define NSGIF_TRAILER 0x3b

/**
 * Convert an LZW result code to equivalent GIF result code.
 *
 * \param[in]  l_res  LZW response code.
 * \return GIF result code.
 */
static nsgif_error nsgif__error_from_lzw(lzw_result l_res)
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
 * Updates the sprite memory size
 *
 * \param gif The animation context
 * \param width The width of the sprite
 * \param height The height of the sprite
 * \return NSGIF_ERR_OOM for a memory error NSGIF_OK for success
 */
static nsgif_error nsgif__initialise_sprite(
		struct nsgif *gif,
		uint32_t width,
		uint32_t height)
{
	/* Already allocated? */
	if (gif->frame_image) {
		return NSGIF_OK;
	}

	assert(gif->bitmap.create);
	gif->frame_image = gif->bitmap.create(width, height);
	if (gif->frame_image == NULL) {
		return NSGIF_ERR_OOM;
	}

	return NSGIF_OK;
}

/**
 * Helper to get the rendering bitmap for a gif.
 *
 * \param[in]  gif  The gif object we're decoding.
 * \return Client pixel buffer for rendering into.
 */
static inline uint32_t* nsgif__bitmap_get(
		struct nsgif *gif)
{
	nsgif_error ret;

	/* Make sure we have a buffer to decode to. */
	ret = nsgif__initialise_sprite(gif, gif->info.width, gif->info.height);
	if (ret != NSGIF_OK) {
		return NULL;
	}

	gif->rowspan = gif->info.width;
	if (gif->bitmap.get_rowspan) {
		gif->rowspan = gif->bitmap.get_rowspan(gif->frame_image);
	}

	/* Get the frame data */
	assert(gif->bitmap.get_buffer);
	return (void *)gif->bitmap.get_buffer(gif->frame_image);
}

/**
 * Helper to tell the client that their bitmap was modified.
 *
 * \param[in]  gif  The gif object we're decoding.
 */
static inline void nsgif__bitmap_modified(
		const struct nsgif *gif)
{
	if (gif->bitmap.modified) {
		gif->bitmap.modified(gif->frame_image);
	}
}

/**
 * Helper to tell the client that whether the bitmap is opaque.
 *
 * \param[in]  gif    The gif object we're decoding.
 * \param[in]  frame  The frame that has been decoded.
 */
static inline void nsgif__bitmap_set_opaque(
		const struct nsgif *gif,
		const struct nsgif_frame *frame)
{
	if (gif->bitmap.set_opaque) {
		gif->bitmap.set_opaque(
				gif->frame_image, frame->opaque);
	}
}

/**
 * Helper to get the client to determine if the bitmap is opaque.
 *
 * \todo: We don't really need to get the client to do this for us.
 *
 * \param[in]  gif    The gif object we're decoding.
 * \return true if the bitmap is opaque, false otherwise.
 */
static inline bool nsgif__bitmap_get_opaque(
		const struct nsgif *gif)
{
	if (gif->bitmap.test_opaque) {
		return gif->bitmap.test_opaque(
				gif->frame_image);
	}

	return false;
}

static void nsgif__record_frame(
		struct nsgif *gif,
		const uint32_t *bitmap)
{
	size_t pixel_bytes = sizeof(*bitmap);
	size_t height = gif->info.height;
	size_t width  = gif->info.width;
	uint32_t *prev_frame;

	if (gif->decoded_frame == NSGIF_FRAME_INVALID ||
	    gif->decoded_frame == gif->prev_index) {
		/* No frame to copy, or already have this frame recorded. */
		return;
	}

	bitmap = nsgif__bitmap_get(gif);
	if (bitmap == NULL) {
		return;
	}

	if (gif->prev_frame == NULL) {
		prev_frame = realloc(gif->prev_frame,
				width * height * pixel_bytes);
		if (prev_frame == NULL) {
			return;
		}
	} else {
		prev_frame = gif->prev_frame;
	}

	memcpy(prev_frame, bitmap, width * height * pixel_bytes);

	gif->prev_frame  = prev_frame;
	gif->prev_index  = gif->decoded_frame;
}

static nsgif_error nsgif__recover_frame(
		const struct nsgif *gif,
		uint32_t *bitmap)
{
	const struct nsgif_canvas canvas = {
		.bitmap = bitmap,
		.width = gif->info.width,
		.height = gif->info.height,
		.rowspan = gif->rowspan,
		.background = gif->info.background,
	};

	nsgif_frame_recover(&canvas, gif->prev_frame);

	return NSGIF_OK;
}

/**
 * Decode a frame's image data into the bitmap (nsgif_frame_pixels_decode()).
 *
 * \param[in] gif         The gif object we're decoding.
 * \param[in] frame       The frame to decode.
 * \param[in] data        The frame's image data (minimum code size) in gif->buf.
 * \param[in] frame_data  The bitmap to decode into.
 * \return NSGIF_OK on success, appropriate error otherwise.
 */
static inline nsgif_error nsgif__decode(
		struct nsgif *gif,
		struct nsgif_frame *frame,
		const uint8_t *data,
		uint32_t *restrict frame_data)
{
	const struct nsgif_canvas canvas = {
		.bitmap = frame_data,
		.width = gif->info.width,
		.height = gif->info.height,
		.rowspan = gif->rowspan,
		.background = gif->info.background,
	};
	const struct nsgif_frame_area area = {
		.rect = frame->info.rect,
		.interlaced = frame->info.interlaced,
		.display = frame->info.display,
		.transparency = frame->info.transparency,
		.transparency_index = frame->transparency_index,
	};

	return nsgif_frame_pixels_decode(gif->lzw_ctx, gif->buf, gif->buf_len,
			data - gif->buf, &canvas, &area, gif->colour_table,
			gif->data_complete);
}

/**
 * Restore a GIF to the background colour (nsgif_frame_restore_bg()).
 *
 * \param[in] gif     The gif object we're decoding.
 * \param[in] frame   The frame to clear, or NULL.
 * \param[in] bitmap  The bitmap to clear the frame in.
 */
static void nsgif__restore_bg(
		struct nsgif *gif,
		struct nsgif_frame *frame,
		uint32_t *bitmap)
{
	const struct nsgif_canvas canvas = {
		.bitmap = bitmap,
		.width = gif->info.width,
		.height = gif->info.height,
		.rowspan = gif->rowspan,
		.background = gif->info.background,
	};

	if (frame == NULL) {
		nsgif_frame_restore_bg(&canvas, NULL);
	} else {
		const struct nsgif_frame_area area = {
			.rect = frame->info.rect,
			.interlaced = frame->info.interlaced,
			.display = frame->info.display,
			.transparency = frame->info.transparency,
			.transparency_index = frame->transparency_index,
		};

		nsgif_frame_restore_bg(&canvas, &area);
	}
}

static nsgif_error nsgif__update_bitmap(
		struct nsgif *gif,
		struct nsgif_frame *frame,
		const uint8_t *data,
		uint32_t frame_idx)
{
	nsgif_error ret;
	uint32_t *bitmap;

	gif->decoded_frame = frame_idx;

	bitmap = nsgif__bitmap_get(gif);
	if (bitmap == NULL) {
		return NSGIF_ERR_OOM;
	}

	/* Handle any bitmap clearing/restoration required before decoding this
	 * frame. */
	if (frame_idx == 0 || gif->decoded_frame == NSGIF_FRAME_INVALID) {
		nsgif__restore_bg(gif, NULL, bitmap);

	} else {
		struct nsgif_frame *prev = &gif->frames[frame_idx - 1];

		if (prev->info.disposal == NSGIF_DISPOSAL_RESTORE_BG) {
			nsgif__restore_bg(gif, prev, bitmap);

		} else if (prev->info.disposal == NSGIF_DISPOSAL_RESTORE_PREV) {
			ret = nsgif__recover_frame(gif, bitmap);
			if (ret != NSGIF_OK) {
				nsgif__restore_bg(gif, prev, bitmap);
			}
		}
	}

	if (frame->info.disposal == NSGIF_DISPOSAL_RESTORE_PREV) {
		/* Store the previous frame for later restoration */
		nsgif__record_frame(gif, bitmap);
	}

	ret = nsgif__decode(gif, frame, data, bitmap);

	nsgif__bitmap_modified(gif);

	if (!frame->decoded) {
		frame->opaque = nsgif__bitmap_get_opaque(gif);
		frame->decoded = true;
	}
	nsgif__bitmap_set_opaque(gif, frame);

	return ret;
}

/**
 * Parse the frame's extensions
 *
 * The parser is nsgif_blocks_extensions() (gif-blocks.c). It writes the
 * graphic control fields and the loop count as it parses, also before an
 * error, so they are copied back in every case.
 *
 * \param[in] gif     The gif object we're decoding.
 * \param[in] frame   The frame to parse extensions for.
 * \param[in] pos     Current position in data, updated on exit.
 * \param[in] decode  Whether to decode or skip over the extension.
 * \return NSGIF_ERR_END_OF_DATA if more data is needed,
 *         NSGIF_OK for success.
 */
static nsgif_error nsgif__parse_frame_extensions(
		struct nsgif *gif,
		struct nsgif_frame *frame,
		const uint8_t **pos,
		bool decode)
{
	size_t p = *pos - gif->buf;
	struct nsgif_blocks_frame_ext ext = {
		.delay = frame->info.delay,
		.disposal = frame->info.disposal,
		.transparency = frame->info.transparency,
		.transparency_index = frame->transparency_index,
		.redraw_required = frame->redraw_required,
	};
	nsgif_error ret;

	ret = nsgif_blocks_extensions(gif->buf, gif->buf_len, &p, decode,
			&ext, &gif->info.loop_max);

	frame->info.delay = ext.delay;
	frame->info.disposal = ext.disposal;
	frame->info.transparency = ext.transparency;
	frame->transparency_index = ext.transparency_index;
	frame->redraw_required = ext.redraw_required;

	if (ret == NSGIF_OK) {
		*pos = gif->buf + p;
	}
	return ret;
}

/**
 * Parse a GIF Image Descriptor (nsgif_blocks_image_descriptor()).
 *
 * \param[in] gif     The gif object we're decoding.
 * \param[in] frame   The frame to parse an image descriptor for.
 * \param[in] pos     Current position in data, updated on exit.
 * \param[in] decode  Whether to decode the image descriptor.
 * \return NSGIF_OK on success, appropriate error otherwise.
 */
static nsgif_error nsgif__parse_image_descriptor(
		struct nsgif *gif,
		struct nsgif_frame *frame,
		const uint8_t **pos,
		bool decode)
{
	size_t p = *pos - gif->buf;
	struct nsgif_blocks_image image;
	nsgif_error ret;

	assert(gif != NULL);
	assert(frame != NULL);

	ret = nsgif_blocks_image_descriptor(gif->buf, gif->buf_len, &p,
			decode, gif->info.frame_count == 0, &image,
			&gif->info.width, &gif->info.height);
	if (ret != NSGIF_OK) {
		return ret;
	}

	if (decode) {
		frame->flags = image.flags;
		frame->info.rect = image.rect;
		frame->info.interlaced = image.interlaced;
	}

	*pos = gif->buf + p;
	return NSGIF_OK;
}

/**
 * Get a frame's colour table.
 *
 * Sets up gif->colour_table for the frame.
 *
 * \param[in] gif     The gif object we're decoding.
 * \param[in] frame   The frame to get the colour table for.
 * \param[in] pos     Current position in data, updated on exit.
 * \param[in] decode  Whether to decode the colour table.
 * \return NSGIF_OK on success, appropriate error otherwise.
 */
static nsgif_error nsgif__parse_colour_table(
		struct nsgif *gif,
		struct nsgif_frame *frame,
		const uint8_t **pos,
		bool decode)
{
	nsgif_error ret;
	const uint8_t *data = *pos;
	size_t len = gif->buf + gif->buf_len - data;
	size_t used_bytes;

	assert(gif != NULL);
	assert(frame != NULL);

	if ((frame->flags & NSGIF_COLOUR_TABLE_MASK) == 0) {
		gif->colour_table = gif->global_colour_table;
		return NSGIF_OK;
	}

	if (decode == false) {
		frame->colour_table_offset = *pos - gif->buf;
	}

	ret = nsgif_blocks_colour_table(
			gif->local_colour_table, &gif->colour_layout,
			2 << (frame->flags & NSGIF_COLOUR_TABLE_SIZE_MASK),
			data, len, &used_bytes, decode);
	if (ret != NSGIF_OK) {
		return ret;
	}
	*pos += used_bytes;

	if (decode) {
		gif->colour_table = gif->local_colour_table;
	} else {
		frame->info.local_palette = true;
	}

	return NSGIF_OK;
}

/**
 * Parse the image data for a gif frame.
 *
 * Sets up gif->colour_table for the frame.
 *
 * \param[in] gif     The gif object we're decoding.
 * \param[in] frame   The frame to parse image data for.
 * \param[in] pos     Current position in data, updated on exit.
 * \param[in] decode  Whether to decode the image data.
 * \return NSGIF_OK on success, appropriate error otherwise.
 */
static nsgif_error nsgif__parse_image_data(
		struct nsgif *gif,
		struct nsgif_frame *frame,
		const uint8_t **pos,
		bool decode)
{
	const uint8_t *data = *pos;
	size_t len = gif->buf + gif->buf_len - data;
	uint32_t frame_idx = frame - gif->frames;
	uint8_t minimum_code_size;
	nsgif_error ret;

	assert(gif != NULL);
	assert(frame != NULL);

	if (!decode) {
		gif->frame_count_partial = frame_idx + 1;
	}

	/* Ensure sufficient data remains.  A gif trailer or a minimum lzw code
	 * followed by a gif trailer is treated as OK, although without any
	 * image data. */
	switch (len) {
		default: if (data[0] == NSGIF_TRAILER) return NSGIF_OK;
			break;
		case 2: if (data[1] == NSGIF_TRAILER) return NSGIF_OK;
			/* Fall through. */
		case 1: if (data[0] == NSGIF_TRAILER) return NSGIF_OK;
			/* Fall through. */
		case 0: return NSGIF_ERR_END_OF_DATA;
	}

	minimum_code_size = data[0];
	if (minimum_code_size >= LZW_CODE_MAX) {
		return NSGIF_ERR_DATA_FRAME;
	}

	if (decode) {
		ret = nsgif__update_bitmap(gif, frame, data, frame_idx);
	} else {
		uint32_t block_size = 0;

		/* Skip the minimum code size. */
		data++;
		len--;

		while (block_size != 1) {
			if (len < 1) {
				return NSGIF_ERR_END_OF_DATA;
			}
			block_size = data[0] + 1;
			/* Check if the frame data runs off the end of the file */
			if (block_size > len) {
				frame->lzw_data_length += len;
				return NSGIF_ERR_END_OF_DATA;
			}

			len -= block_size;
			data += block_size;
			frame->lzw_data_length += block_size;
		}

		*pos = data;

		gif->info.frame_count = frame_idx + 1;
		gif->frames[frame_idx].info.display = true;

		return NSGIF_OK;
	}

	return ret;
}

static struct nsgif_frame *nsgif__get_frame(
		struct nsgif *gif,
		uint32_t frame_idx)
{
	struct nsgif_frame *frame;

	if (gif->frame_holders > frame_idx) {
		frame = &gif->frames[frame_idx];
	} else {
		/* Allocate more memory */
		size_t count = frame_idx + 1;
		struct nsgif_frame *temp;

		temp = realloc(gif->frames, count * sizeof(*frame));
		if (temp == NULL) {
			return NULL;
		}
		gif->frames = temp;
		gif->frame_holders = count;

		frame = &gif->frames[frame_idx];

		frame->info.local_palette = false;
		frame->info.transparency = false;
		frame->info.display = false;
		frame->info.disposal = 0;
		frame->info.delay = 10;

		frame->transparency_index = NSGIF_NO_TRANSPARENCY;
		frame->frame_offset = gif->buf_pos;
		frame->redraw_required = false;
		frame->lzw_data_length = 0;
		frame->decoded = false;
	}

	return frame;
}

/**
 * Attempts to initialise the next frame
 *
 * \param[in] gif       The animation context
 * \param[in] frame_idx The frame number to decode.
 * \param[in] decode    Whether to decode the graphical image data.
 * \return NSGIF_OK on success, appropriate error otherwise.
*/
static nsgif_error nsgif__process_frame(
		struct nsgif *gif,
		uint32_t frame_idx,
		bool decode)
{
	nsgif_error ret;
	const uint8_t *pos;
	const uint8_t *end;
	struct nsgif_frame *frame;

	frame = nsgif__get_frame(gif, frame_idx);
	if (frame == NULL) {
		return NSGIF_ERR_OOM;
	}

	end = gif->buf + gif->buf_len;

	if (decode) {
		pos = gif->buf + frame->frame_offset;

		/* Ensure this frame is supposed to be decoded */
		if (frame->info.display == false) {
			return NSGIF_OK;
		}

		/* Ensure the frame is in range to decode */
		if (frame_idx > gif->frame_count_partial) {
			return NSGIF_ERR_END_OF_DATA;
		}

		/* Done if frame is already decoded */
		if (frame_idx == gif->decoded_frame) {
			return NSGIF_OK;
		}
	} else {
		pos = gif->buf + gif->buf_pos;

		/* Check if we've finished */
		if (pos < end && pos[0] == NSGIF_TRAILER) {
			return NSGIF_OK;
		}
	}

	ret = nsgif__parse_frame_extensions(gif, frame, &pos, !decode);
	if (ret != NSGIF_OK) {
		goto cleanup;
	}

	ret = nsgif__parse_image_descriptor(gif, frame, &pos, !decode);
	if (ret != NSGIF_OK) {
		goto cleanup;
	}

	ret = nsgif__parse_colour_table(gif, frame, &pos, decode);
	if (ret != NSGIF_OK) {
		goto cleanup;
	}

	ret = nsgif__parse_image_data(gif, frame, &pos, decode);
	if (ret != NSGIF_OK) {
		goto cleanup;
	}

cleanup:
	if (!decode) {
		gif->buf_pos = pos - gif->buf;
	}

	return ret;
}

/* exported function documented in nsgif.h */
void nsgif_destroy(nsgif_t *gif)
{
	if (gif == NULL) {
		return;
	}

	/* Release all our memory blocks */
	if (gif->frame_image) {
		assert(gif->bitmap.destroy);
		gif->bitmap.destroy(gif->frame_image);
		gif->frame_image = NULL;
	}

	free(gif->frames);
	gif->frames = NULL;

	free(gif->prev_frame);
	gif->prev_frame = NULL;

	lzw_context_destroy(gif->lzw_ctx);
	gif->lzw_ctx = NULL;

	free(gif);
}

/**
 * Check whether the host is little endian.
 *
 * Checks whether least significant bit is in the first byte of a `uint16_t`.
 *
 * \return true if host is little endian.
 */
static inline bool nsgif__host_is_little_endian(void)
{
	const uint16_t test = 1;

	return ((const uint8_t *) &test)[0];
}

static struct nsgif_colour_layout nsgif__bitmap_fmt_to_colour_layout(
		nsgif_bitmap_fmt_t bitmap_fmt)
{
	bool le = nsgif__host_is_little_endian();

	/* Map endian-dependant formats to byte-wise format for the host. */
	switch (bitmap_fmt) {
	case NSGIF_BITMAP_FMT_RGBA8888:
		bitmap_fmt = (le) ? NSGIF_BITMAP_FMT_A8B8G8R8
		                  : NSGIF_BITMAP_FMT_R8G8B8A8;
		break;
	case NSGIF_BITMAP_FMT_BGRA8888:
		bitmap_fmt = (le) ? NSGIF_BITMAP_FMT_A8R8G8B8
		                  : NSGIF_BITMAP_FMT_B8G8R8A8;
		break;
	case NSGIF_BITMAP_FMT_ARGB8888:
		bitmap_fmt = (le) ? NSGIF_BITMAP_FMT_B8G8R8A8
		                  : NSGIF_BITMAP_FMT_A8R8G8B8;
		break;
	case NSGIF_BITMAP_FMT_ABGR8888:
		bitmap_fmt = (le) ? NSGIF_BITMAP_FMT_R8G8B8A8
		                  : NSGIF_BITMAP_FMT_A8B8G8R8;
		break;
	default:
		break;
	}

	/* Set up colour component order for bitmap format. */
	switch (bitmap_fmt) {
	default:
		/* Fall through. */
	case NSGIF_BITMAP_FMT_R8G8B8A8:
		return (struct nsgif_colour_layout) {
			.r = 0,
			.g = 1,
			.b = 2,
			.a = 3,
		};

	case NSGIF_BITMAP_FMT_B8G8R8A8:
		return (struct nsgif_colour_layout) {
			.b = 0,
			.g = 1,
			.r = 2,
			.a = 3,
		};

	case NSGIF_BITMAP_FMT_A8R8G8B8:
		return (struct nsgif_colour_layout) {
			.a = 0,
			.r = 1,
			.g = 2,
			.b = 3,
		};

	case NSGIF_BITMAP_FMT_A8B8G8R8:
		return (struct nsgif_colour_layout) {
			.a = 0,
			.b = 1,
			.g = 2,
			.r = 3,
		};
	}
}

/* exported function documented in nsgif.h */
nsgif_error nsgif_create(
		const nsgif_bitmap_cb_vt *bitmap_vt,
		nsgif_bitmap_fmt_t bitmap_fmt,
		nsgif_t **gif_out)
{
	nsgif_t *gif;

	gif = calloc(1, sizeof(*gif));
	if (gif == NULL) {
		return NSGIF_ERR_OOM;
	}

	gif->bitmap = *bitmap_vt;
	gif->decoded_frame = NSGIF_FRAME_INVALID;
	gif->prev_index = NSGIF_FRAME_INVALID;

	gif->delay_min = NSGIF_FRAME_DELAY_MIN;
	gif->delay_default = NSGIF_FRAME_DELAY_DEFAULT;

	gif->colour_layout = nsgif__bitmap_fmt_to_colour_layout(bitmap_fmt);

	*gif_out = gif;
	return NSGIF_OK;
}

/* exported function documented in nsgif.h */
void nsgif_set_frame_delay_behaviour(
		nsgif_t *gif,
		uint16_t delay_min,
		uint16_t delay_default)
{
	gif->delay_min = delay_min;
	gif->delay_default = delay_default;
}

/**
 * Read GIF header (nsgif_blocks_header()).
 *
 * \param[in]      gif     The GIF object we're decoding.
 * \param[in,out]  pos     The current buffer position, updated on success.
 * \param[in]      strict  Whether to require a known GIF version.
 * \return NSGIF_OK on success, appropriate error otherwise.
 */
static nsgif_error nsgif__parse_header(
		struct nsgif *gif,
		const uint8_t **pos,
		bool strict)
{
	size_t p = *pos - gif->buf;
	nsgif_error ret;

	ret = nsgif_blocks_header(gif->buf, gif->buf_len, &p, strict);
	if (ret != NSGIF_OK) {
		return ret;
	}

	*pos = gif->buf + p;
	return NSGIF_OK;
}

/**
 * Read Logical Screen Descriptor (nsgif_blocks_screen()).
 *
 * \param[in]      gif     The GIF object we're decoding.
 * \param[in,out]  pos     The current buffer position, updated on success.
 * \return NSGIF_OK on success, appropriate error otherwise.
 */
static nsgif_error nsgif__parse_logical_screen_descriptor(
		struct nsgif *gif,
		const uint8_t **pos)
{
	size_t p = *pos - gif->buf;
	struct nsgif_screen screen;
	nsgif_error ret;

	ret = nsgif_blocks_screen(gif->buf, gif->buf_len, &p, &screen);
	if (ret != NSGIF_OK) {
		return ret;
	}

	gif->info.width = screen.width;
	gif->info.height = screen.height;
	gif->info.global_palette = screen.global_palette;
	gif->colour_table_size = screen.colour_table_size;
	gif->bg_index = screen.bg_index;
	gif->aspect_ratio = screen.aspect_ratio;
	gif->info.loop_max = screen.loop_max;

	*pos = gif->buf + p;
	return NSGIF_OK;
}

/* exported function documented in nsgif.h */
nsgif_error nsgif_data_scan(
		nsgif_t *gif,
		size_t size,
		const uint8_t *data)
{
	const uint8_t *nsgif_data;
	nsgif_error ret;
	uint32_t frames;

	if (gif->data_complete) {
		return NSGIF_ERR_DATA_COMPLETE;
	}

	/* Initialize values */
	gif->buf_len = size;
	gif->buf = data;

	/* Get our current processing position */
	nsgif_data = gif->buf + gif->buf_pos;

	/* See if we should initialise the GIF */
	if (gif->buf_pos == 0) {
		/* We want everything to be NULL before we start so we've no
		 * chance of freeing bad pointers (paranoia)
		 */
		gif->frame_image = NULL;
		gif->frames = NULL;
		gif->frame_holders = 0;

		/* The caller may have been lazy and not reset any values */
		gif->info.frame_count = 0;
		gif->frame_count_partial = 0;
		gif->decoded_frame = NSGIF_FRAME_INVALID;
		gif->frame = NSGIF_FRAME_INVALID;

		ret = nsgif__parse_header(gif, &nsgif_data, false);
		if (ret != NSGIF_OK) {
			return ret;
		}

		ret = nsgif__parse_logical_screen_descriptor(gif, &nsgif_data);
		if (ret != NSGIF_OK) {
			return ret;
		}

		/* Remember we've done this now */
		gif->buf_pos = nsgif_data - gif->buf;

		/* Broken GIFs that report a screen size get 1x1. */
		nsgif_blocks_screen_size_quirk(
				&gif->info.width, &gif->info.height);

		/* Set the first colour to a value that will never occur in
		 * reality so we know if we've processed it
		*/
		gif->global_colour_table[0] = NSGIF_PROCESS_COLOURS;

		/* Check if the GIF has no frame data (13-byte header + 1-byte
		 * termination block) Although generally useless, the GIF
		 * specification does not expressly prohibit this
		 */
		if (gif->buf_len == gif->buf_pos + 1) {
			if (nsgif_data[0] == NSGIF_TRAILER) {
				return NSGIF_OK;
			}
		}
	}

	/*  Do the colour map if we haven't already. As the top byte is always
	 *  0xff or 0x00 depending on the transparency we know if it's been
	 *  filled in.
	 */
	if (gif->global_colour_table[0] == NSGIF_PROCESS_COLOURS) {
		/* Check for a global colour map signified by bit 7 */
		if (gif->info.global_palette) {
			size_t remaining = gif->buf + gif->buf_len - nsgif_data;
			size_t used;

			ret = nsgif_blocks_colour_table(
					gif->global_colour_table,
					&gif->colour_layout,
					gif->colour_table_size,
					nsgif_data, remaining, &used, true);
			if (ret != NSGIF_OK) {
				return ret;
			}

			nsgif_data += used;
			gif->buf_pos = (nsgif_data - gif->buf);
		} else {
			/* Create a default colour table with the first two
			 * colours as black and white. */
			uint8_t *entry = (uint8_t *)gif->global_colour_table;

			/* Black */
			entry[gif->colour_layout.r] = 0x00;
			entry[gif->colour_layout.g] = 0x00;
			entry[gif->colour_layout.b] = 0x00;
			entry[gif->colour_layout.a] = 0xFF;

			entry += sizeof(uint32_t);

			/* White */
			entry[gif->colour_layout.r] = 0xFF;
			entry[gif->colour_layout.g] = 0xFF;
			entry[gif->colour_layout.b] = 0xFF;
			entry[gif->colour_layout.a] = 0xFF;

			gif->colour_table_size = 2;
		}

		if (gif->info.global_palette &&
		    gif->bg_index < gif->colour_table_size) {
			size_t bg_idx = gif->bg_index;
			gif->info.background = gif->global_colour_table[bg_idx];
		} else {
			gif->info.background = gif->global_colour_table[0];
		}
	}

	if (gif->lzw_ctx == NULL) {
		lzw_result res = lzw_context_create(
				(struct lzw_ctx **)&gif->lzw_ctx);
		if (res != LZW_OK) {
			return nsgif__error_from_lzw(res);
		}
	}

	/* Try to initialise all frames. */
	do {
		frames = gif->info.frame_count;
		ret = nsgif__process_frame(gif, frames, false);
	} while (gif->info.frame_count > frames);

	if (ret == NSGIF_ERR_END_OF_DATA && gif->info.frame_count > 0) {
		ret = NSGIF_OK;
	}

	return ret;
}

/* exported function documented in nsgif.h */
void nsgif_data_complete(
		nsgif_t *gif)
{
	if (gif->data_complete == false) {
		uint32_t start = gif->info.frame_count;
		uint32_t end = gif->frame_count_partial;

		for (uint32_t f = start; f < end; f++) {
			nsgif_frame *frame = &gif->frames[f];

			if (frame->lzw_data_length > 0) {
				frame->info.display = true;
				gif->info.frame_count = f + 1;

				if (f == 0) {
					frame->info.transparency = true;
				}
				break;
			}
		}
	}

	gif->data_complete = true;
}

static void nsgif__redraw_rect_extend(
		const nsgif_rect_t *frame,
		nsgif_rect_t *redraw)
{
	if (redraw->x1 == 0 || redraw->y1 == 0) {
		*redraw = *frame;
	} else {
		if (redraw->x0 > frame->x0) {
			redraw->x0 = frame->x0;
		}
		if (redraw->x1 < frame->x1) {
			redraw->x1 = frame->x1;
		}
		if (redraw->y0 > frame->y0) {
			redraw->y0 = frame->y0;
		}
		if (redraw->y1 < frame->y1) {
			redraw->y1 = frame->y1;
		}
	}
}

static uint32_t nsgif__frame_next(
		const nsgif_t *gif,
		bool partial,
		uint32_t frame)
{
	uint32_t frames = partial ?
			gif->frame_count_partial :
			gif->info.frame_count;

	if (frames == 0) {
		return NSGIF_FRAME_INVALID;
	}

	frame++;
	return (frame >= frames) ? 0 : frame;
}

static nsgif_error nsgif__next_displayable_frame(
		const nsgif_t *gif,
		uint32_t *frame,
		uint32_t *delay)
{
	uint32_t next = *frame;

	do {
		next = nsgif__frame_next(gif, false, next);
		if (next <= *frame && *frame != NSGIF_FRAME_INVALID &&
				gif->data_complete == false) {
			return NSGIF_ERR_END_OF_DATA;

		} else if (next == *frame || next == NSGIF_FRAME_INVALID) {
			return NSGIF_ERR_FRAME_DISPLAY;
		}

		if (delay != NULL) {
			*delay += gif->frames[next].info.delay;
		}

	} while (gif->frames[next].info.display == false);

	*frame = next;
	return NSGIF_OK;
}

static inline bool nsgif__animation_complete(int count, int max)
{
	if (max == 0) {
		return false;
	}

	return (count >= max);
}

nsgif_error nsgif_reset(
		nsgif_t *gif)
{
	gif->loop_count = 0;
	gif->frame = NSGIF_FRAME_INVALID;

	return NSGIF_OK;
}

/* exported function documented in nsgif.h */
nsgif_error nsgif_frame_prepare(
		nsgif_t *gif,
		nsgif_rect_t *area,
		uint32_t *delay_cs,
		uint32_t *frame_new)
{
	nsgif_error ret;
	nsgif_rect_t rect = {
		.x1 = 0,
		.y1 = 0,
	};
	uint32_t delay = 0;
	uint32_t frame = gif->frame;

	if (gif->frame != NSGIF_FRAME_INVALID &&
	    gif->frame < gif->info.frame_count &&
	    gif->frames[gif->frame].info.display) {
		rect = gif->frames[gif->frame].info.rect;
	}

	if (nsgif__animation_complete(
			gif->loop_count,
			gif->info.loop_max)) {
		return NSGIF_ERR_ANIMATION_END;
	}

	ret = nsgif__next_displayable_frame(gif, &frame, &delay);
	if (ret != NSGIF_OK) {
		return ret;
	}

	if (gif->frame != NSGIF_FRAME_INVALID && frame < gif->frame) {
		gif->loop_count++;
	}

	if (gif->data_complete) {
		/* Check for last frame, which has infinite delay. */

		if (gif->info.frame_count == 1) {
			delay = NSGIF_INFINITE;
		} else if (gif->info.loop_max != 0) {
			uint32_t frame_next = frame;

			ret = nsgif__next_displayable_frame(gif,
					&frame_next, NULL);
			if (ret != NSGIF_OK) {
				return ret;
			}

			if (gif->data_complete && frame_next < frame) {
				if (nsgif__animation_complete(
						gif->loop_count + 1,
						gif->info.loop_max)) {
					delay = NSGIF_INFINITE;
				}
			}
		}
	}

	gif->frame = frame;
	nsgif__redraw_rect_extend(&gif->frames[frame].info.rect, &rect);

	if (delay < gif->delay_min) {
		delay = gif->delay_default;
	}

	*frame_new = gif->frame;
	*delay_cs = delay;
	*area = rect;

	return NSGIF_OK;
}

/* exported function documented in nsgif.h */
nsgif_error nsgif_frame_decode(
		nsgif_t *gif,
		uint32_t frame,
		nsgif_bitmap_t **bitmap)
{
	uint32_t start_frame;
	nsgif_error ret = NSGIF_OK;

	if (frame >= gif->info.frame_count) {
		return NSGIF_ERR_BAD_FRAME;
	}

	if (gif->decoded_frame == frame) {
		*bitmap = gif->frame_image;
		return NSGIF_OK;

	} else if (gif->decoded_frame >= frame ||
	           gif->decoded_frame == NSGIF_FRAME_INVALID) {
		/* Can skip to first frame or restart. */
		start_frame = 0;
	} else {
		start_frame = nsgif__frame_next(
				gif, false, gif->decoded_frame);
	}

	for (uint32_t f = start_frame; f <= frame; f++) {
		ret = nsgif__process_frame(gif, f, true);
		if (ret != NSGIF_OK) {
			return ret;
		}
	}

	*bitmap = gif->frame_image;
	return ret;
}

/* exported function documented in nsgif.h */
const nsgif_info_t *nsgif_get_info(const nsgif_t *gif)
{
	return &gif->info;
}

/* exported function documented in nsgif.h */
const nsgif_frame_info_t *nsgif_get_frame_info(
		const nsgif_t *gif,
		uint32_t frame)
{
	if (frame >= gif->info.frame_count) {
		return NULL;
	}

	return &gif->frames[frame].info;
}

/* exported function documented in nsgif.h */
void nsgif_global_palette(
		const nsgif_t *gif,
		uint32_t table[NSGIF_MAX_COLOURS],
		size_t *entries)
{
	size_t len = sizeof(*table) * NSGIF_MAX_COLOURS;

	memcpy(table, gif->global_colour_table, len);
	*entries = gif->colour_table_size;
}

/* exported function documented in nsgif.h */
bool nsgif_local_palette(
		const nsgif_t *gif,
		uint32_t frame,
		uint32_t table[NSGIF_MAX_COLOURS],
		size_t *entries)
{
	const nsgif_frame *f;

	if (frame >= gif->frame_count_partial) {
		return false;
	}

	f = &gif->frames[frame];
	if (f->info.local_palette == false) {
		return false;
	}

	*entries = 2 << (f->flags & NSGIF_COLOUR_TABLE_SIZE_MASK);
	nsgif_blocks_colour_table_decode(table, &gif->colour_layout,
			*entries, gif->buf + f->colour_table_offset);

	return true;
}

/* exported function documented in nsgif.h */
const char *nsgif_strerror(nsgif_error err)
{
	static const char *const str[] = {
		[NSGIF_OK]                = "Success",
		[NSGIF_ERR_OOM]           = "Out of memory",
		[NSGIF_ERR_DATA]          = "Invalid source data",
		[NSGIF_ERR_BAD_FRAME]     = "Requested frame does not exist",
		[NSGIF_ERR_DATA_FRAME]    = "Invalid frame data",
		[NSGIF_ERR_END_OF_DATA]   = "Unexpected end of GIF source data",
		[NSGIF_ERR_DATA_COMPLETE] = "Can't add data to completed GIF",
		[NSGIF_ERR_FRAME_DISPLAY] = "Frame can't be displayed",
		[NSGIF_ERR_ANIMATION_END] = "Animation complete",
	};

	if (err >= NSGIF_ARRAY_LEN(str) || str[err] == NULL) {
		return "Unknown error";
	}

	return str[err];
}

/* exported function documented in nsgif.h */
const char *nsgif_str_disposal(enum nsgif_disposal disposal)
{
	static const char *const str[] = {
		[NSGIF_DISPOSAL_UNSPECIFIED]   = "Unspecified",
		[NSGIF_DISPOSAL_NONE]          = "None",
		[NSGIF_DISPOSAL_RESTORE_BG]    = "Restore background",
		[NSGIF_DISPOSAL_RESTORE_PREV]  = "Restore previous",
		[NSGIF_DISPOSAL_RESTORE_QUIRK] = "Restore quirk",
	};

	if (disposal >= NSGIF_ARRAY_LEN(str) || str[disposal] == NULL) {
		return "Unspecified";
	}

	return str[disposal];
}

/* File-local macros (BUILD.md): out of the next file in the unity group. */
#undef NSGIF_FRAME_DELAY_MIN
#undef NSGIF_FRAME_DELAY_DEFAULT
#undef NSGIF_ARRAY_LEN
#undef NSGIF_PROCESS_COLOURS
#undef NSGIF_FRAME_INVALID
#undef NSGIF_NO_TRANSPARENCY
#undef NSGIF_COLOUR_TABLE_MASK
#undef NSGIF_COLOUR_TABLE_SIZE_MASK
#undef NSGIF_TRAILER
