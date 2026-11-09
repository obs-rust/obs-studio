#include "libobs/build/libobs.pch.h"

void obs_encoder_set_preferred_video_format(obs_encoder_t *encoder, enum video_format format)
{
	if (!encoder || encoder->info.type != OBS_ENCODER_VIDEO)
		return;

	encoder->preferred_format = format;
}
