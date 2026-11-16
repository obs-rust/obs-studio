#include "libobs/build/libobs-graphics.pch.h"

void gs_enable_stencil_test(bool enable)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_enable_stencil_test"))
		return;

	graphics->exports.device_enable_stencil_test(graphics->device, enable);
}

void gs_enable_stencil_write(bool enable)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_enable_stencil_write"))
		return;

	graphics->exports.device_enable_stencil_write(graphics->device, enable);
}

void gs_stencil_function(enum gs_stencil_side side, enum gs_depth_test test)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_stencil_function"))
		return;

	graphics->exports.device_stencil_function(graphics->device, side, test);
}

void gs_stencil_op(enum gs_stencil_side side, enum gs_stencil_op_type fail, enum gs_stencil_op_type zfail,
		   enum gs_stencil_op_type zpass)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_stencil_op"))
		return;

	graphics->exports.device_stencil_op(graphics->device, side, fail, zfail, zpass);
}
