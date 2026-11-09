#include "libobs/build/libobs-graphics.pch.h"

gs_indexbuffer_t *gs_indexbuffer_create(enum gs_index_type type, void *indices, size_t num, uint32_t flags)
{
	graphics_t *graphics = thread_graphics;

	if (!gs_valid("gs_indexbuffer_create"))
		return NULL;

	if (indices && num && (flags & GS_DUP_BUFFER) != 0) {
		size_t size = type == GS_UNSIGNED_SHORT ? 2 : 4;
		indices = bmemdup(indices, size * num);
	}

	return graphics->exports.device_indexbuffer_create(graphics->device, type, indices, num, flags);
}
