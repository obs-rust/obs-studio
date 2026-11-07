#include "libobs/build/libobs.pch.h"

bool obs_encoder_scaling_enabled(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_scaling_enabled"))
		return false;

	return obs_encoder_size_scaling(encoder->scaled_width, encoder->scaled_height);
}
