#include "libobs/build/libobs.pch.h"

bool obs_encoder_gpu_scaling_enabled(obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_gpu_scaling_enabled"))
		return 0;
	if (encoder->info.type != OBS_ENCODER_VIDEO) {
		blog(LOG_WARNING,
		     "obs_encoder_gpu_scaling_enabled: "
		     "encoder '%s' is not a video encoder",
		     obs_encoder_get_name(encoder));
		return 0;
	}

	return encoder->gpu_scale_type != OBS_SCALE_DISABLE;
}

enum obs_scale_type obs_encoder_get_scale_type(obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_get_scale_type"))
		return 0;
	if (encoder->info.type != OBS_ENCODER_VIDEO) {
		blog(LOG_WARNING,
		     "obs_encoder_get_scale_type: "
		     "encoder '%s' is not a video encoder",
		     obs_encoder_get_name(encoder));
		return 0;
	}

	return encoder->gpu_scale_type;
}
