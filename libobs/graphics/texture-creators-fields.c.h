#include "libobs/build/libobs-ported-graphics.pch.h"

/* Only private layout access and backend dispatch live here. */
gs_texture_t *obs_texture_creator_dispatch(graphics_t *context, uint32_t width, uint32_t height,
					   enum gs_color_format format, uint32_t levels, const uint8_t **data,
					   uint32_t flags, bool cube);
void obs_texture_creator_log(int level, const char *text);

gs_texture_t *obs_texture_creator_dispatch(graphics_t *context, uint32_t width, uint32_t height,
					   enum gs_color_format format, uint32_t levels, const uint8_t **data,
					   uint32_t flags, bool cube)
{
	if (cube)
		return context->exports.device_cubetexture_create(context->device, width, format, levels, data, flags);
	return context->exports.device_texture_create(context->device, width, height, format, levels, data, flags);
}

void obs_texture_creator_log(int level, const char *text)
{
	blog(level, "%s", text);
}
