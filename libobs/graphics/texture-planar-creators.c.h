#include "libobs/build/libobs-graphics.pch.h"

bool gs_texture_create_nv12(gs_texture_t **tex_y, gs_texture_t **tex_uv, uint32_t width, uint32_t height,
			    uint32_t flags)
{
	graphics_t *graphics = thread_graphics;
	bool success = false;

	if (!gs_valid("gs_texture_create_nv12"))
		return false;

	if ((width & 1) == 1 || (height & 1) == 1) {
		blog(LOG_ERROR, "NV12 textures must have dimensions "
				"divisible by 2.");
		return false;
	}

	if (graphics->exports.device_texture_create_nv12) {
		success = graphics->exports.device_texture_create_nv12(graphics->device, tex_y, tex_uv, width, height,
								       flags);
		if (success)
			return true;
	}

	*tex_y = gs_texture_create(width, height, GS_R8, 1, NULL, flags);
	*tex_uv = gs_texture_create(width / 2, height / 2, GS_R8G8, 1, NULL, flags);

	if (!*tex_y || !*tex_uv) {
		if (*tex_y)
			gs_texture_destroy(*tex_y);
		if (*tex_uv)
			gs_texture_destroy(*tex_uv);
		*tex_y = NULL;
		*tex_uv = NULL;
		return false;
	}

	return true;
}

bool gs_texture_create_p010(gs_texture_t **tex_y, gs_texture_t **tex_uv, uint32_t width, uint32_t height,
			    uint32_t flags)
{
	graphics_t *graphics = thread_graphics;
	bool success = false;

	if (!gs_valid("gs_texture_create_p010"))
		return false;

	if ((width & 1) == 1 || (height & 1) == 1) {
		blog(LOG_ERROR, "P010 textures must have dimensions "
				"divisible by 2.");
		return false;
	}

	if (graphics->exports.device_texture_create_p010) {
		success = graphics->exports.device_texture_create_p010(graphics->device, tex_y, tex_uv, width, height,
								       flags);
		if (success)
			return true;
	}

	*tex_y = gs_texture_create(width, height, GS_R16, 1, NULL, flags);
	*tex_uv = gs_texture_create(width / 2, height / 2, GS_RG16, 1, NULL, flags);

	if (!*tex_y || !*tex_uv) {
		if (*tex_y)
			gs_texture_destroy(*tex_y);
		if (*tex_uv)
			gs_texture_destroy(*tex_uv);
		*tex_y = NULL;
		*tex_uv = NULL;
		return false;
	}

	return true;
}
