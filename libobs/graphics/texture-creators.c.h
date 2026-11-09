#include "libobs/build/libobs-graphics.pch.h"

gs_texture_t *gs_texture_create(uint32_t width, uint32_t height, enum gs_color_format color_format, uint32_t levels,
				const uint8_t **data, uint32_t flags)
{
	graphics_t *graphics = thread_graphics;
	struct gs_texture_mip_plan plan = gs_texture_mip_plan(width, height, flags, levels);

	if (!gs_valid("gs_texture_create"))
		return NULL;

	switch (plan.warning) {
	case GS_TEXTURE_MIP_NOT_POW2:
		blog(LOG_WARNING, "Cannot use mipmaps with a "
				  "non-power-of-two texture.  Disabling "
				  "mipmaps for this texture.");
		break;
	case GS_TEXTURE_MIP_RENDER_TARGET:
		blog(LOG_WARNING, "Cannot use mipmaps with render targets.  "
				  "Disabling mipmaps for this texture.");
		break;
	case GS_TEXTURE_MIP_OK:
		break;
	}

	return graphics->exports.device_texture_create(graphics->device, width, height, color_format, plan.levels, data,
						       plan.flags);
}

gs_texture_t *gs_cubetexture_create(uint32_t size, enum gs_color_format color_format, uint32_t levels,
				    const uint8_t **data, uint32_t flags)
{
	graphics_t *graphics = thread_graphics;
	/* The 2D rule of a size x size texture. */
	struct gs_texture_mip_plan plan = gs_texture_mip_plan(size, size, flags, levels);

	if (!gs_valid("gs_cubetexture_create"))
		return NULL;

	switch (plan.warning) {
	case GS_TEXTURE_MIP_NOT_POW2:
		blog(LOG_WARNING, "Cannot use mipmaps with a "
				  "non-power-of-two texture.  Disabling "
				  "mipmaps for this texture.");
		break;
	case GS_TEXTURE_MIP_RENDER_TARGET:
		blog(LOG_WARNING, "Cannot use mipmaps with render targets.  "
				  "Disabling mipmaps for this texture.");
		/* Unlike gs_texture_create(), which keeps it: the devices
		 * read the data face by face, each face with all the levels
		 * asked for, so with one level faces 1-5 would read the wrong
		 * entries. A 2D texture's entry 0 stays its top level. */
		data = NULL;
		break;
	case GS_TEXTURE_MIP_OK:
		break;
	}

	return graphics->exports.device_cubetexture_create(graphics->device, size, color_format, plan.levels, data,
							   plan.flags);
}
