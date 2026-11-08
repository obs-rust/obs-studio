#include "libobs/build/libobs.pch.h"

#define encoder_active(encoder) os_atomic_load_bool(&encoder->active)

bool obs_encoder_active(const obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_encoder_active") ? encoder_active(encoder) : false;
}

bool obs_encoder_paused(const obs_encoder_t *encoder)
{
	return obs_encoder_valid(encoder, "obs_encoder_paused") ? os_atomic_load_bool(&encoder->paused) : false;
}

#undef encoder_active
