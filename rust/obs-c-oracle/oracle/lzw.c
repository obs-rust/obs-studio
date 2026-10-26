/* Test-only oracle: the original libobs/graphics/libnsgif/lzw.c.h,
 * unmodified, with its six functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

#define lzw_context_create oracle_lzw_context_create
#define lzw_context_destroy oracle_lzw_context_destroy
#define lzw_decode_init oracle_lzw_decode_init
#define lzw_decode oracle_lzw_decode
#define lzw_decode_init_map oracle_lzw_decode_init_map
#define lzw_decode_map oracle_lzw_decode_map

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
#include "graphics/libnsgif/lzw.c.h"
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

/* Fill a context with one byte right after creation: the C context is
 * malloc'd and never cleared, so the parity test pins what the C would read
 * from it (0x00, as the Rust context starts) and finds where it does
 * (another byte). */
void oracle_lzw_context_fill(struct lzw_ctx *ctx, int byte);

void oracle_lzw_context_fill(struct lzw_ctx *ctx, int byte)
{
	memset(ctx, byte, sizeof(*ctx));
}

/* The values the C uses, in the order of rust/obs-c-oracle/src/lzw.rs. */
enum {
	ORACLE_LZW_LAYOUT_COUNT = 11,
};

size_t oracle_lzw_layout(size_t which);

size_t oracle_lzw_layout(size_t which)
{
	const size_t values[ORACLE_LZW_LAYOUT_COUNT] = {
		sizeof(lzw_result), LZW_OK,        LZW_OK_EOD,    LZW_NO_MEM,   LZW_NO_DATA,  LZW_EOI_CODE,
		LZW_NO_COLOUR,      LZW_BAD_ICODE, LZW_BAD_PARAM, LZW_BAD_CODE, LZW_CODE_MAX,
	};
	return which < ORACLE_LZW_LAYOUT_COUNT ? values[which] : 0;
}
