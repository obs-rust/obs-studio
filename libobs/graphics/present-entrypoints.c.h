#include "libobs/build/libobs-graphics.pch.h"

void gs_begin_frame(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_begin_frame"))
		return;

	graphics->exports.device_begin_frame(graphics->device);
}

void gs_begin_scene(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_begin_scene"))
		return;

	graphics->exports.device_begin_scene(graphics->device);
}

void gs_draw(enum gs_draw_mode draw_mode, uint32_t start_vert, uint32_t num_verts)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_draw"))
		return;

	graphics->exports.device_draw(graphics->device, draw_mode, start_vert, num_verts);
}

void gs_end_scene(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_end_scene"))
		return;

	graphics->exports.device_end_scene(graphics->device);
}

void gs_load_swapchain(gs_swapchain_t *swapchain)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_load_swapchain"))
		return;

	graphics->exports.device_load_swapchain(graphics->device, swapchain);
}

void gs_clear(uint32_t clear_flags, const struct vec4 *color, float depth, uint8_t stencil)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_clear"))
		return;

	graphics->exports.device_clear(graphics->device, clear_flags, color, depth, stencil);
}

bool gs_is_present_ready(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_is_present_ready"))
		return false;

	return graphics->exports.device_is_present_ready(graphics->device);
}

void gs_present(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_present"))
		return;

	graphics->exports.device_present(graphics->device);
}

void gs_flush(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_flush"))
		return;

	graphics->exports.device_flush(graphics->device);
}
