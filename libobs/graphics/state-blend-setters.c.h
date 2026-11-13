#include "libobs/build/libobs-ported-blend-entrypoints.pch.h"

void gs_enable_blending(bool enable)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_enable_blending"))
		return;

	graphics->cur_blend_state.enabled = enable;
	graphics->exports.device_enable_blending(graphics->device, enable);
}

void gs_blend_function(enum gs_blend_type src, enum gs_blend_type dest)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_blend_function"))
		return;

	graphics->cur_blend_state.src_c = src;
	graphics->cur_blend_state.dest_c = dest;
	graphics->cur_blend_state.src_a = src;
	graphics->cur_blend_state.dest_a = dest;
	graphics->exports.device_blend_function(graphics->device, src, dest);
}

void gs_blend_function_separate(enum gs_blend_type src_c, enum gs_blend_type dest_c, enum gs_blend_type src_a,
				enum gs_blend_type dest_a)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_blend_function_separate"))
		return;

	graphics->cur_blend_state.src_c = src_c;
	graphics->cur_blend_state.dest_c = dest_c;
	graphics->cur_blend_state.src_a = src_a;
	graphics->cur_blend_state.dest_a = dest_a;
	graphics->exports.device_blend_function_separate(graphics->device, src_c, dest_c, src_a, dest_a);
}

void gs_blend_op(enum gs_blend_op_type op)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_blend_op"))
		return;

	graphics->cur_blend_state.op = op;
	graphics->exports.device_blend_op(graphics->device, graphics->cur_blend_state.op);
}
