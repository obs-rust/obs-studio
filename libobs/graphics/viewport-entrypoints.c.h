#include "libobs/build/libobs-graphics.pch.h"

/* File-local copies of graphics.c.h's pointer checks, which it undefines
 * at its end (BUILD.md). */
#define ptr_valid(ptr, func) gs_obj_valid(ptr, func, #ptr)
#define gs_valid_p(func, param1) (gs_valid(func) && ptr_valid(param1, func))

void gs_set_viewport(int x, int y, int width, int height)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_set_viewport"))
		return;

	graphics->exports.device_set_viewport(graphics->device, x, y, width, height);
}

void gs_get_viewport(struct gs_rect *rect)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid_p("gs_get_viewport", rect))
		return;

	graphics->exports.device_get_viewport(graphics->device, rect);
}

void gs_set_scissor_rect(const struct gs_rect *rect)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_set_scissor_rect"))
		return;

	graphics->exports.device_set_scissor_rect(graphics->device, rect);
}

void gs_ortho(float left, float right, float top, float bottom, float znear, float zfar)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_ortho"))
		return;

	graphics->exports.device_ortho(graphics->device, left, right, top, bottom, znear, zfar);
}

void gs_frustum(float left, float right, float top, float bottom, float znear, float zfar)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_frustum"))
		return;

	graphics->exports.device_frustum(graphics->device, left, right, top, bottom, znear, zfar);
}

void gs_projection_push(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_projection_push"))
		return;

	graphics->exports.device_projection_push(graphics->device);
}

void gs_projection_pop(void)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_projection_pop"))
		return;

	graphics->exports.device_projection_pop(graphics->device);
}

#undef ptr_valid
#undef gs_valid_p
