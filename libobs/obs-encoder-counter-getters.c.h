#include "libobs/build/libobs-ported-service-getters.pch.h"

uint32_t obs_encoder_get_encoded_frames(const obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_output_get_encoded_frames") ? encoder->encoded_frames : 0;
}

uint64_t obs_encoder_get_pause_offset(const obs_encoder_t *encoder)
{
	return encoder ? encoder->pause.ts_offset : 0;
}
