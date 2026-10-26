/* Test-only oracle: the original libobs/graphics/libnsgif/gif-blocks.c.h,
 * unmodified, with its seven functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <assert.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define nsgif_blocks_header oracle_nsgif_blocks_header
#define nsgif_blocks_screen oracle_nsgif_blocks_screen
#define nsgif_blocks_screen_size_quirk oracle_nsgif_blocks_screen_size_quirk
#define nsgif_blocks_extensions oracle_nsgif_blocks_extensions
#define nsgif_blocks_image_descriptor oracle_nsgif_blocks_image_descriptor
#define nsgif_blocks_colour_table oracle_nsgif_blocks_colour_table
#define nsgif_blocks_colour_table_decode oracle_nsgif_blocks_colour_table_decode

/* Third-party code: not part of the warning ratchet (#516), as the
 * obs_c_oracle_nsgif library in build.rs. */
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#pragma GCC diagnostic ignored "-Wconversion"
#ifdef __clang__
#pragma GCC diagnostic ignored "-Wimplicit-int-conversion"
#endif
#elif defined(_MSC_VER)
#pragma warning(push)
#pragma warning(disable : 4244 4267)
#endif
#include "graphics/libnsgif/gif-blocks.c.h"
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic pop
#elif defined(_MSC_VER)
#pragma warning(pop)
#endif

#undef nsgif_blocks_colour_table_decode
#undef nsgif_blocks_colour_table
#undef nsgif_blocks_image_descriptor
#undef nsgif_blocks_extensions
#undef nsgif_blocks_screen_size_quirk
#undef nsgif_blocks_screen
#undef nsgif_blocks_header

/* The layout of gif-blocks.h's types, in the order of
 * rust/obs-c-oracle/src/gif_blocks.rs. */
enum {
	ORACLE_GIF_BLOCKS_LAYOUT_COUNT = 31,
};

size_t oracle_gif_blocks_layout(size_t which);

size_t oracle_gif_blocks_layout(size_t which)
{
	const size_t values[ORACLE_GIF_BLOCKS_LAYOUT_COUNT] = {
		sizeof(nsgif_error),
		NSGIF_OK,
		NSGIF_ERR_OOM,
		NSGIF_ERR_DATA,
		NSGIF_ERR_BAD_FRAME,
		NSGIF_ERR_DATA_FRAME,
		NSGIF_ERR_END_OF_DATA,
		NSGIF_ERR_DATA_COMPLETE,
		NSGIF_ERR_FRAME_DISPLAY,
		NSGIF_ERR_ANIMATION_END,
		NSGIF_MAX_COLOURS,
		sizeof(struct nsgif_colour_layout),
		sizeof(struct nsgif_screen),
		offsetof(struct nsgif_screen, height),
		offsetof(struct nsgif_screen, global_palette),
		offsetof(struct nsgif_screen, colour_table_size),
		offsetof(struct nsgif_screen, bg_index),
		offsetof(struct nsgif_screen, aspect_ratio),
		offsetof(struct nsgif_screen, loop_max),
		sizeof(struct nsgif_blocks_frame_ext),
		offsetof(struct nsgif_blocks_frame_ext, disposal),
		offsetof(struct nsgif_blocks_frame_ext, transparency),
		offsetof(struct nsgif_blocks_frame_ext, transparency_index),
		offsetof(struct nsgif_blocks_frame_ext, redraw_required),
		sizeof(struct nsgif_blocks_image),
		offsetof(struct nsgif_blocks_image, rect),
		offsetof(struct nsgif_blocks_image, interlaced),
		sizeof(nsgif_rect_t),
		NSGIF_DISPOSAL_RESTORE_BG,
		NSGIF_DISPOSAL_RESTORE_PREV,
		NSGIF_DISPOSAL_RESTORE_QUIRK,
	};
	return which < ORACLE_GIF_BLOCKS_LAYOUT_COUNT ? values[which] : 0;
}
