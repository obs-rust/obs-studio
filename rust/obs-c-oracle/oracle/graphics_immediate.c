/* Test-only oracle: the original libobs/graphics/graphics-immediate.c.h,
 * unmodified, with its four functions renamed to oracle_* so they can link
 * next to the Rust shims. Every input is defined in C. The draw and the
 * save are renamed by function-like macros, which leave the struct tags of
 * the same names alone. */
#include <stddef.h>

#define gs_immediate_unit_ok oracle_gs_immediate_unit_ok
#define gs_immediate_push_ok oracle_gs_immediate_push_ok
#define gs_immediate_draw(...) oracle_gs_immediate_draw(__VA_ARGS__)
#define gs_immediate_save(...) oracle_gs_immediate_save(__VA_ARGS__)

#include "graphics/graphics-immediate.c.h"

#undef gs_immediate_save
#undef gs_immediate_draw
#undef gs_immediate_push_ok
#undef gs_immediate_unit_ok

/* The layout of the header's types and its constants, in the order of
 * rust/obs-c-oracle/src/graphics_immediate.rs. */
enum {
	ORACLE_GS_IMMEDIATE_LAYOUT_COUNT = 11,
};

size_t oracle_gs_immediate_layout(size_t which);

size_t oracle_gs_immediate_layout(size_t which)
{
	const size_t values[ORACLE_GS_IMMEDIATE_LAYOUT_COUNT] = {
		sizeof(struct gs_immediate_draw),
		offsetof(struct gs_immediate_draw, num),
		offsetof(struct gs_immediate_draw, norms_mismatch),
		offsetof(struct gs_immediate_draw, colors_mismatch),
		offsetof(struct gs_immediate_draw, tex_mismatch),
		sizeof(struct gs_immediate_save),
		offsetof(struct gs_immediate_save, num),
		offsetof(struct gs_immediate_save, num_tex),
		offsetof(struct gs_immediate_save, free_from),
		GS_IMMEDIATE_COUNT,
		GS_IMMEDIATE_TEX_ARRAYS,
	};
	return which < ORACLE_GS_IMMEDIATE_LAYOUT_COUNT ? values[which] : 0;
}
