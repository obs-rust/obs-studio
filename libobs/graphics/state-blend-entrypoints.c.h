#include "libobs/build/libobs-graphics.pch.h"

void gs_blend_state_push(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_blend_state_push"))
		return;

	gs_blend_state_stack_push(&graphics->blend_state_stack.da, &graphics->cur_blend_state);
}

void gs_blend_state_pop(void)
{
	graphics_t *graphics = thread_graphics;
	struct blend_state state;

	if (!gs_valid("gs_blend_state_pop"))
		return;

	if (!gs_blend_state_stack_pop(&graphics->blend_state_stack.da, &state))
		return;

	gs_enable_blending(state.enabled);
	gs_blend_function_separate(state.src_c, state.dest_c, state.src_a, state.dest_a);
	gs_blend_op(state.op);
}

void gs_reset_blend_state(void)
{
	graphics_t *graphics = thread_graphics;
	struct gs_blend_reset steps;

	if (!gs_valid("gs_reset_blend_state"))
		return;

	steps = gs_blend_state_reset_steps(&graphics->cur_blend_state);
	if (steps.enable)
		gs_enable_blending(true);
	if (steps.function)
		gs_blend_function_separate(GS_BLEND_SRCALPHA, GS_BLEND_INVSRCALPHA, GS_BLEND_ONE, GS_BLEND_INVSRCALPHA);
	if (steps.op)
		gs_blend_op(GS_BLEND_OP_ADD);
}
