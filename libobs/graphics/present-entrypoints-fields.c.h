#include "libobs/build/libobs-graphics.pch.h"

/* Private graphics layout, the debug line and the device ABI for the Rust
 * frame and present calls (obs-graphics::present_entrypoints): the context
 * read, gs_valid's debug line, and one device dispatch per call. */
graphics_t *obs_graphics_present_context(void);
graphics_t *obs_graphics_present_context(void)
{
	return gs_get_context();
}

void obs_graphics_present_debug(const char *function);
void obs_graphics_present_debug(const char *function)
{
	blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
}

void obs_graphics_present_begin_frame(graphics_t *graphics);
void obs_graphics_present_begin_frame(graphics_t *graphics)
{
	graphics->exports.device_begin_frame(graphics->device);
}

void obs_graphics_present_begin_scene(graphics_t *graphics);
void obs_graphics_present_begin_scene(graphics_t *graphics)
{
	graphics->exports.device_begin_scene(graphics->device);
}

void obs_graphics_present_draw(graphics_t *graphics, enum gs_draw_mode draw_mode, uint32_t start_vert,
			       uint32_t num_verts);
void obs_graphics_present_draw(graphics_t *graphics, enum gs_draw_mode draw_mode, uint32_t start_vert,
			       uint32_t num_verts)
{
	graphics->exports.device_draw(graphics->device, draw_mode, start_vert, num_verts);
}

void obs_graphics_present_end_scene(graphics_t *graphics);
void obs_graphics_present_end_scene(graphics_t *graphics)
{
	graphics->exports.device_end_scene(graphics->device);
}

void obs_graphics_present_load_swapchain(graphics_t *graphics, gs_swapchain_t *swapchain);
void obs_graphics_present_load_swapchain(graphics_t *graphics, gs_swapchain_t *swapchain)
{
	graphics->exports.device_load_swapchain(graphics->device, swapchain);
}

void obs_graphics_present_clear(graphics_t *graphics, uint32_t clear_flags, const struct vec4 *color, float depth,
				uint8_t stencil);
void obs_graphics_present_clear(graphics_t *graphics, uint32_t clear_flags, const struct vec4 *color, float depth,
				uint8_t stencil)
{
	graphics->exports.device_clear(graphics->device, clear_flags, color, depth, stencil);
}

bool obs_graphics_present_is_present_ready(graphics_t *graphics);
bool obs_graphics_present_is_present_ready(graphics_t *graphics)
{
	return graphics->exports.device_is_present_ready(graphics->device);
}

void obs_graphics_present_present(graphics_t *graphics);
void obs_graphics_present_present(graphics_t *graphics)
{
	graphics->exports.device_present(graphics->device);
}

void obs_graphics_present_flush(graphics_t *graphics);
void obs_graphics_present_flush(graphics_t *graphics)
{
	graphics->exports.device_flush(graphics->device);
}
