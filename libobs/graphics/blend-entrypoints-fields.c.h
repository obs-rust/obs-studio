#include "libobs/build/libobs-graphics.pch.h"

/* Private layout access only; Rust owns the entry-point orchestration. */
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
