/* Test-only oracle: the original libobs/graphics/graphics-immediate.c.h,
 * unmodified, with its six functions renamed to oracle_* so they can link
 * next to the Rust shims. Every input is defined in C. */
#include <stddef.h>

#define gs_immediate_cut oracle_gs_immediate_cut
#define gs_immediate_mismatch oracle_gs_immediate_mismatch
#define gs_immediate_push_ok oracle_gs_immediate_push_ok
#define gs_immediate_save_cut oracle_gs_immediate_save_cut
#define gs_immediate_save_free_from oracle_gs_immediate_save_free_from
#define gs_immediate_unit_ok oracle_gs_immediate_unit_ok

#include "graphics/graphics-immediate.c.h"

#undef gs_immediate_unit_ok
#undef gs_immediate_save_free_from
#undef gs_immediate_save_cut
#undef gs_immediate_push_ok
#undef gs_immediate_mismatch
#undef gs_immediate_cut

/* The header's constants and the sizes and alignments of the types that
 * cross, in the order of rust/obs-c-oracle/src/graphics_immediate.rs. */
enum {
	ORACLE_GS_IMMEDIATE_LAYOUT_COUNT = 8,
};

size_t oracle_gs_immediate_layout(size_t which);

size_t oracle_gs_immediate_layout(size_t which)
{
	const size_t values[ORACLE_GS_IMMEDIATE_LAYOUT_COUNT] = {
		GS_IMMEDIATE_COUNT, GS_IMMEDIATE_TEX_ARRAYS, sizeof(size_t), _Alignof(size_t),
		sizeof(bool),       _Alignof(bool),          sizeof(int),    _Alignof(int),
	};
	return which < ORACLE_GS_IMMEDIATE_LAYOUT_COUNT ? values[which] : 0;
}
