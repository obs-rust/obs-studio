#include "libobs/build/libobs.pch.h"

void obs_encoder_set_preferred_color_space(obs_encoder_t *encoder, enum video_colorspace colorspace)
{
	if (!encoder || encoder->info.type != OBS_ENCODER_VIDEO)
		return;

	encoder->preferred_space = colorspace;
}

void obs_encoder_set_preferred_range(obs_encoder_t *encoder, enum video_range_type range)
{
	if (!encoder || encoder->info.type != OBS_ENCODER_VIDEO)
		return;

	encoder->preferred_range = range;
}
