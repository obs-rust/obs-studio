/* Test-only oracle: the original libobs/graphics/libnsgif/gif-frame.c.h,
 * unmodified, with its three functions renamed to oracle_* so they can link
 * next to the Rust shims. Its LZW calls go to the C LZW oracle
 * (oracle/lzw.c), so the whole decode is the C's. */
#include <assert.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define nsgif_frame_pixels_decode oracle_nsgif_frame_pixels_decode
#define nsgif_frame_restore_bg oracle_nsgif_frame_restore_bg
#define nsgif_frame_recover oracle_nsgif_frame_recover
#define lzw_context_create oracle_lzw_context_create
#define lzw_context_destroy oracle_lzw_context_destroy
#define lzw_decode_init oracle_lzw_decode_init
#define lzw_decode oracle_lzw_decode
#define lzw_decode_init_map oracle_lzw_decode_init_map
#define lzw_decode_map oracle_lzw_decode_map

/* Third-party code: not part of the warning ratchet (#516), as the
 * obs_c_oracle_nsgif library in build.rs. MSVC's -W4 also reports
 * 'uncompressed' in nsgif_frame__decode_complex as maybe uninitialized
 * (C4701/C4703): it is set by lzw_decode before the first read, because
 * available starts at 0. */
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#pragma GCC diagnostic ignored "-Wconversion"
#ifdef __clang__
#pragma GCC diagnostic ignored "-Wimplicit-int-conversion"
#endif
#elif defined(_MSC_VER)
#pragma warning(push)
#pragma warning(disable : 4244 4267 4701 4703)
#endif
#include "graphics/libnsgif/gif-frame.c.h"
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic pop
#elif defined(_MSC_VER)
#pragma warning(pop)
#endif

#undef lzw_decode_map
#undef lzw_decode_init_map
#undef lzw_decode
#undef lzw_decode_init
#undef lzw_context_destroy
#undef lzw_context_create
#undef nsgif_frame_recover
#undef nsgif_frame_restore_bg
#undef nsgif_frame_pixels_decode

/* The layout of gif-frame.h's types, in the order of
 * rust/obs-c-oracle/src/gif_frame.rs. */
enum {
	ORACLE_GIF_FRAME_LAYOUT_COUNT = 11,
};

size_t oracle_gif_frame_layout(size_t which);

size_t oracle_gif_frame_layout(size_t which)
{
	const size_t values[ORACLE_GIF_FRAME_LAYOUT_COUNT] = {
		sizeof(struct nsgif_canvas),
		offsetof(struct nsgif_canvas, width),
		offsetof(struct nsgif_canvas, height),
		offsetof(struct nsgif_canvas, rowspan),
		offsetof(struct nsgif_canvas, background),
		sizeof(struct nsgif_frame_area),
		offsetof(struct nsgif_frame_area, rect),
		offsetof(struct nsgif_frame_area, interlaced),
		offsetof(struct nsgif_frame_area, display),
		offsetof(struct nsgif_frame_area, transparency),
		offsetof(struct nsgif_frame_area, transparency_index),
	};
	return which < ORACLE_GIF_FRAME_LAYOUT_COUNT ? values[which] : 0;
}
