#include "libobs/build/libobs-ported-service-getters.pch.h"

enum video_format obs_encoder_get_preferred_video_format(const obs_encoder_t *encoder)
{
	if (!encoder || encoder->info.type != OBS_ENCODER_VIDEO)
		return VIDEO_FORMAT_NONE;

	return encoder->preferred_format;
}
