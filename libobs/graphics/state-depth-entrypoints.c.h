#include "libobs/build/libobs-graphics.pch.h"

void gs_enable_depth_test(bool enable)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_enable_depth_test"))
		return;

	graphics->exports.device_enable_depth_test(graphics->device, enable);
}

void gs_depth_function(enum gs_depth_test test)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_depth_function"))
		return;

	graphics->exports.device_depth_function(graphics->device, test);
}
