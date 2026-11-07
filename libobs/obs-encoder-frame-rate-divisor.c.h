#include "libobs/build/libobs.pch.h"

uint32_t obs_encoder_get_frame_rate_divisor(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_set_frame_rate_divisor"))
		return 0;

	if (encoder->info.type != OBS_ENCODER_VIDEO) {
		blog(LOG_WARNING,
		     "obs_encoder_set_frame_rate_divisor: "
		     "encoder '%s' is not a video encoder",
		     obs_encoder_get_name(encoder));
		return 0;
	}

	return encoder->frame_rate_divisor;
}
