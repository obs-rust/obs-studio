#include "libobs/build/libobs.pch.h"

void set_deinterlace_texture_size(obs_source_t *source)
{
	const enum gs_color_format format = convert_video_format(source->async_format, source->async_trc);

	if (source->async_gpu_conversion) {
		source->async_prev_texrender = gs_texrender_create(format, GS_ZS_NONE);

		for (int c = 0; c < source->async_channel_count; c++)
			source->async_prev_textures[c] =
				gs_texture_create(source->async_convert_width[c], source->async_convert_height[c],
						  source->async_texture_formats[c], 1, NULL, GS_DYNAMIC);
	} else {
		source->async_prev_textures[0] =
			gs_texture_create(source->async_width, source->async_height, format, 1, NULL, GS_DYNAMIC);
	}
}
