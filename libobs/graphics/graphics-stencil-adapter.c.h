#include "libobs/build/libobs-ported-graphics.pch.h"

/* C owns the private context layout, validation log and callback signatures. */
void *obs_graphics_stencil_context(const char *function);
void obs_graphics_stencil_test(void *context, bool enable);
void obs_graphics_stencil_write(void *context, bool enable);
void obs_graphics_stencil_function(void *context, int side, int test);
void obs_graphics_stencil_op(void *context, int side, int fail, int zfail, int zpass);

void *obs_graphics_stencil_context(const char *function)
{
	graphics_t *graphics = gs_get_context();
	if (!graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", function);
		return NULL;
	}
	return graphics;
}

void obs_graphics_stencil_test(void *context, bool enable)
{
	graphics_t *graphics = context;
	graphics->exports.device_enable_stencil_test(graphics->device, enable);
}

void obs_graphics_stencil_write(void *context, bool enable)
{
	graphics_t *graphics = context;
	graphics->exports.device_enable_stencil_write(graphics->device, enable);
}

void obs_graphics_stencil_function(void *context, int side, int test)
{
	graphics_t *graphics = context;
	graphics->exports.device_stencil_function(graphics->device, (enum gs_stencil_side)side,
						  (enum gs_depth_test)test);
}

void obs_graphics_stencil_op(void *context, int side, int fail, int zfail, int zpass)
{
	graphics_t *graphics = context;
	graphics->exports.device_stencil_op(graphics->device, (enum gs_stencil_side)side, (enum gs_stencil_op_type)fail,
					    (enum gs_stencil_op_type)zfail, (enum gs_stencil_op_type)zpass);
}
