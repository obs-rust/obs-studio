/* Test-only oracle: the original libobs/graphics/libnsgif/gif.c.h,
 * unmodified, with its public functions renamed to oracle_* so they can
 * link next to the Rust shims. Its calls into the other libnsgif files go
 * to their C oracles (oracle/gif_blocks.c, oracle/gif_frame.c,
 * oracle/lzw.c), so the whole of libnsgif here is the C's. */
#include <assert.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define nsgif_create oracle_nsgif_create
#define nsgif_destroy oracle_nsgif_destroy
#define nsgif_set_frame_delay_behaviour oracle_nsgif_set_frame_delay_behaviour
#define nsgif_data_scan oracle_nsgif_data_scan
#define nsgif_data_complete oracle_nsgif_data_complete
#define nsgif_reset oracle_nsgif_reset
#define nsgif_frame_prepare oracle_nsgif_frame_prepare
#define nsgif_frame_decode oracle_nsgif_frame_decode
#define nsgif_get_info oracle_nsgif_get_info
#define nsgif_get_frame_info oracle_nsgif_get_frame_info
#define nsgif_global_palette oracle_nsgif_global_palette
#define nsgif_local_palette oracle_nsgif_local_palette
#define nsgif_strerror oracle_nsgif_strerror
#define nsgif_str_disposal oracle_nsgif_str_disposal
#define nsgif_blocks_header oracle_nsgif_blocks_header
#define nsgif_blocks_screen oracle_nsgif_blocks_screen
#define nsgif_blocks_screen_size_quirk oracle_nsgif_blocks_screen_size_quirk
#define nsgif_blocks_extensions oracle_nsgif_blocks_extensions
#define nsgif_blocks_image_descriptor oracle_nsgif_blocks_image_descriptor
#define nsgif_blocks_colour_table oracle_nsgif_blocks_colour_table
#define nsgif_blocks_colour_table_decode oracle_nsgif_blocks_colour_table_decode
#define nsgif_frame_pixels_decode oracle_nsgif_frame_pixels_decode
#define nsgif_frame_restore_bg oracle_nsgif_frame_restore_bg
#define nsgif_frame_recover oracle_nsgif_frame_recover
#define lzw_context_create oracle_lzw_context_create
#define lzw_context_destroy oracle_lzw_context_destroy
#define lzw_decode_init oracle_lzw_decode_init
#define lzw_decode oracle_lzw_decode
#define lzw_decode_init_map oracle_lzw_decode_init_map
#define lzw_decode_map oracle_lzw_decode_map

/* Third-party code: not part of the warning ratchet (#516). */
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic push
#pragma GCC diagnostic ignored "-Wsign-conversion"
#pragma GCC diagnostic ignored "-Wconversion"
#pragma GCC diagnostic ignored "-Wsign-compare"
#pragma GCC diagnostic ignored "-Wimplicit-fallthrough"
#ifdef __clang__
#pragma GCC diagnostic ignored "-Wimplicit-int-conversion"
#endif
#elif defined(_MSC_VER)
#pragma warning(push)
#pragma warning(disable : 4244 4267 4701 4703)
#endif
#include "graphics/libnsgif/gif.c.h"
#if defined(__GNUC__) || defined(__clang__)
#pragma GCC diagnostic pop
#elif defined(_MSC_VER)
#pragma warning(pop)
#endif
