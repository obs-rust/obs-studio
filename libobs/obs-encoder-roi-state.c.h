#include "libobs/build/libobs-ported-service-getters.pch.h"

bool obs_encoder_has_roi(const obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_encoder_has_roi") ? encoder->roi.num > 0 : false;
}

uint32_t obs_encoder_get_roi_increment(const obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_encoder_get_roi_increment") ? encoder->roi_increment : 0;
}
