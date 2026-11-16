#include "libobs/build/libobs-graphics.pch.h"

/* Private layout access and the device ABI only; Rust owns the blend entry
 * points and setters (obs-graphics::blend_entrypoints). */
graphics_t *obs_graphics_blend_context(const char *function);
struct blend_state *obs_graphics_blend_current(graphics_t *graphics);
struct darray *obs_graphics_blend_stack(graphics_t *graphics);

graphics_t *obs_graphics_blend_context(const char *function)
{
	graphics_t *graphics = gs_get_context();
	if (!graphics)
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
	return graphics;
}
struct blend_state *obs_graphics_blend_current(graphics_t *graphics)
{
	return &graphics->cur_blend_state;
}
struct darray *obs_graphics_blend_stack(graphics_t *graphics)
{
	return &graphics->blend_state_stack.da;
}

/* The device's four blend calls, one dispatch each, for the Rust setters. */
void obs_graphics_blend_enable_blending(graphics_t *graphics, bool enable);
void obs_graphics_blend_function(graphics_t *graphics, enum gs_blend_type src, enum gs_blend_type dest);
void obs_graphics_blend_function_separate(graphics_t *graphics, enum gs_blend_type src_c, enum gs_blend_type dest_c,
					  enum gs_blend_type src_a, enum gs_blend_type dest_a);
void obs_graphics_blend_op(graphics_t *graphics, enum gs_blend_op_type op);

void obs_graphics_blend_enable_blending(graphics_t *graphics, bool enable)
{
	graphics->exports.device_enable_blending(graphics->device, enable);
}
void obs_graphics_blend_function(graphics_t *graphics, enum gs_blend_type src, enum gs_blend_type dest)
{
	graphics->exports.device_blend_function(graphics->device, src, dest);
}
void obs_graphics_blend_function_separate(graphics_t *graphics, enum gs_blend_type src_c, enum gs_blend_type dest_c,
					  enum gs_blend_type src_a, enum gs_blend_type dest_a)
{
	graphics->exports.device_blend_function_separate(graphics->device, src_c, dest_c, src_a, dest_a);
}
void obs_graphics_blend_op(graphics_t *graphics, enum gs_blend_op_type op)
{
	graphics->exports.device_blend_op(graphics->device, op);
}
