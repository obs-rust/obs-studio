#include "libobs/build/libobs.pch.h"

const char *obs_encoder_get_name(const obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_encoder_get_name") ? encoder->context.name : NULL;
}
