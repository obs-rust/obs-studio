#include "libobs/build/libobs.pch.h"

struct obs_source_deinterlace_texture_fields obs_source_deinterlace_texture_fields(obs_source_t *source)
{
	return (struct obs_source_deinterlace_texture_fields){
		.format = &source->async_format,
		.trc = &source->async_trc,
		.gpu = &source->async_gpu_conversion,
		.channels = &source->async_channel_count,
		.width = &source->async_width,
		.height = &source->async_height,
		.widths = source->async_convert_width,
		.heights = source->async_convert_height,
		.formats = source->async_texture_formats,
		.textures = source->async_prev_textures,
		.render = &source->async_prev_texrender,
	};
}
