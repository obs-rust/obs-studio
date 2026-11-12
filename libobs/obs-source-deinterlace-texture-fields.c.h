#include "libobs/build/libobs.pch.h"

struct obs_source_deinterlace_texture_fields obs_source_deinterlace_texture_fields(obs_source_t *source)
{
	struct obs_source_deinterlace_texture_fields fields = {
		.format = (int)source->async_format,
		.trc = source->async_trc,
		.gpu = source->async_gpu_conversion,
		.channels = source->async_channel_count,
		.width = source->async_width,
		.height = source->async_height,
		.textures = source->async_prev_textures,
		.render = &source->async_prev_texrender,
	};
	for (size_t c = 0; c < MAX_AV_PLANES; c++) {
		fields.widths[c] = source->async_convert_width[c];
		fields.heights[c] = source->async_convert_height[c];
		fields.formats[c] = (int)source->async_texture_formats[c];
	}
	return fields;
}
