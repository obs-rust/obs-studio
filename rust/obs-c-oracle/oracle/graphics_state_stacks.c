/* Test-only oracle: the original libobs/graphics/graphics-state-stacks.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims. Its matrix4 calls go to the graphics math oracle
 * (graphics_math_names.h), so the stacks here are all C. */
#include "graphics_math_names.h"

#define gs_matrix_stack_push oracle_gs_matrix_stack_push
#define gs_matrix_stack_pop oracle_gs_matrix_stack_pop
#define gs_matrix_stack_identity oracle_gs_matrix_stack_identity
#define gs_matrix_stack_transpose oracle_gs_matrix_stack_transpose
#define gs_matrix_stack_set oracle_gs_matrix_stack_set
#define gs_matrix_stack_get oracle_gs_matrix_stack_get
#define gs_matrix_stack_mul oracle_gs_matrix_stack_mul
#define gs_matrix_stack_rotquat oracle_gs_matrix_stack_rotquat
#define gs_matrix_stack_rotaa oracle_gs_matrix_stack_rotaa
#define gs_matrix_stack_translate oracle_gs_matrix_stack_translate
#define gs_matrix_stack_scale oracle_gs_matrix_stack_scale
#define gs_viewport_stack_push oracle_gs_viewport_stack_push
#define gs_viewport_stack_pop oracle_gs_viewport_stack_pop
#define gs_blend_state_stack_push oracle_gs_blend_state_stack_push
#define gs_blend_state_stack_pop oracle_gs_blend_state_stack_pop
#define gs_blend_state_reset_steps oracle_gs_blend_state_reset_steps

#include "graphics/graphics-state-stacks.c.h"

/* The layout of the header's types, in the order of
 * rust/obs-c-oracle/src/graphics_state_stacks.rs. */
enum {
	ORACLE_GS_STATE_STACKS_LAYOUT_COUNT = 14,
};

size_t oracle_gs_state_stacks_layout(size_t which);

size_t oracle_gs_state_stacks_layout(size_t which)
{
	const size_t values[ORACLE_GS_STATE_STACKS_LAYOUT_COUNT] = {
		sizeof(struct gs_rect),
		sizeof(struct blend_state),
		offsetof(struct blend_state, src_c),
		offsetof(struct blend_state, dest_c),
		offsetof(struct blend_state, src_a),
		offsetof(struct blend_state, dest_a),
		offsetof(struct blend_state, op),
		sizeof(struct gs_blend_reset),
		offsetof(struct gs_blend_reset, function),
		offsetof(struct gs_blend_reset, op),
		sizeof(struct matrix4),
		GS_BLEND_SRCALPHA,
		GS_BLEND_INVSRCALPHA,
		GS_BLEND_OP_ADD,
	};
	return which < ORACLE_GS_STATE_STACKS_LAYOUT_COUNT ? values[which] : 0;
}
