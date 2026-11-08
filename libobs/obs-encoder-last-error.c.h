#include "libobs/build/libobs-ported-service-getters.pch.h"

const char *obs_encoder_get_last_error(obs_encoder_t *encoder)
{
	if (!obs_encoder_valid(encoder, "obs_encoder_get_last_error"))
		return NULL;

	return encoder->last_error_message;
}
