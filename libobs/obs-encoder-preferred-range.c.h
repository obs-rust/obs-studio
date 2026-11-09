#include "libobs/build/libobs.pch.h"

enum video_range_type obs_encoder_get_preferred_range(const obs_encoder_t *encoder)
{
	if (!encoder || encoder->info.type != OBS_ENCODER_VIDEO)
		return VIDEO_RANGE_DEFAULT;

	return encoder->preferred_range;
}
