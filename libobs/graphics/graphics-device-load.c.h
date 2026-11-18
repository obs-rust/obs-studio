#include "libobs/build/libobs-graphics.pch.h"

/* Reference-only group: read graphics.c.h's context through its existing
 * getter, and preserve its validator and each original function body. */
#define thread_graphics gs_get_context()

static inline bool device_load_ref_valid(const char *f)
{
	if (!thread_graphics) {
		blog(LOG_DEBUG, "%s: called while not in a graphics context", f);
		return false;
	}

	return true;
}
#define gs_valid device_load_ref_valid

void gs_load_vertexbuffer(gs_vertbuffer_t *vertbuffer)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_load_vertexbuffer"))
		return;

	graphics->exports.device_load_vertexbuffer(graphics->device, vertbuffer);
}

void gs_load_indexbuffer(gs_indexbuffer_t *indexbuffer)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_load_indexbuffer"))
		return;

	graphics->exports.device_load_indexbuffer(graphics->device, indexbuffer);
}

void gs_load_texture(gs_texture_t *tex, int unit)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_load_texture"))
		return;

	graphics->exports.device_load_texture(graphics->device, tex, unit);
}

void gs_load_samplerstate(gs_samplerstate_t *samplerstate, int unit)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_load_samplerstate"))
		return;

	graphics->exports.device_load_samplerstate(graphics->device, samplerstate, unit);
}

void gs_load_default_samplerstate(bool b_3d, int unit)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_load_default_samplerstate"))
		return;

	graphics->exports.device_load_default_samplerstate(graphics->device, b_3d, unit);
}

#undef gs_valid
#undef thread_graphics
