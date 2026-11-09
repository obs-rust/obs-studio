#include "libobs/build/libobs-ported-service-getters.pch.h"

enum video_colorspace obs_encoder_get_preferred_color_space(const obs_encoder_t *encoder)
{
	if (!encoder || encoder->info.type != OBS_ENCODER_VIDEO)
		return VIDEO_CS_DEFAULT;

	return encoder->preferred_space;
}
