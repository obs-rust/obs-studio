#include "libobs/build/libobs-ported-service-getters.pch.h"

const char *obs_encoder_get_codec(const obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_encoder_get_codec") ? encoder->info.codec : NULL;
}


enum obs_encoder_type obs_encoder_get_type(const obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_encoder_get_type") ? encoder->info.type : OBS_ENCODER_AUDIO;
}


void *obs_encoder_get_type_data(obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_encoder_get_type_data") ? encoder->orig_info.type_data : NULL;
}


const char *obs_encoder_get_id(const obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_encoder_get_id") ? encoder->orig_info.id : NULL;
}


uint32_t obs_encoder_get_caps(const obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_encoder_get_caps") ? encoder->orig_info.caps : 0;
}

