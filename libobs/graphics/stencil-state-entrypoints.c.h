#include "libobs/build/libobs-graphics.pch.h"

/* Standalone C reference: read the private context through its accessor;
 * retain graphics.c.h's validation and the extracted dispatch bodies. */
#define thread_graphics gs_get_context()

static inline bool stencil_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}
#define gs_valid stencil_ref_valid

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

#undef gs_valid
#undef thread_graphics
