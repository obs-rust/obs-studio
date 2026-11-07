#include "libobs/build/libobs.pch.h"

struct obs_encoder_last_error_fields obs_encoder_last_error_fields(obs_encoder_t *encoder)
{
	return (struct obs_encoder_last_error_fields){encoder ? &encoder->last_error_message : NULL};
}
