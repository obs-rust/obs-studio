#include "libobs/build/libobs.pch.h"

struct obs_encoder_configuration_fields obs_encoder_configuration_fields(obs_encoder_t *encoder)
{
	return (struct obs_encoder_configuration_fields){&encoder->info.type,          &encoder->active,
							 &encoder->initialized,        &encoder->scaled_width,
							 &encoder->scaled_height,      &encoder->gpu_scale_type,
							 &encoder->frame_rate_divisor, encoder->media,
							 &encoder->fps_override,       encoder->context.name};
}

bool obs_encoder_configuration_output_size(void *media, uint32_t *width, uint32_t *height)
{
	const struct video_output_info *info = video_output_get_info(media);
	if (!info)
		return false;
	*width = info->width;
	*height = info->height;
	return true;
}

void obs_encoder_configuration_free_override(void *video)
{
	video_output_free_frame_rate_divisor(video);
}

void *obs_encoder_configuration_create_override(void *media, uint32_t divisor)
{
	return video_output_create_with_frame_rate_divisor(media, divisor);
}
