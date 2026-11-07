#include "libobs/build/libobs-ported-service-getters.pch.h"

/* Existing hidden size helper, shared with the C reference. */
extern bool obs_encoder_size_scaling(uint32_t width, uint32_t height);

bool obs_encoder_scaling_enabled(const obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_scaling_enabled"))
		return false;

	return obs_encoder_size_scaling(encoder->scaled_width, encoder->scaled_height);
}
