#include "libobs/build/libobs.pch.h"

const char *obs_encoder_get_name_context_name(const obs_encoder_t *encoder);

const char *obs_encoder_get_name_context_name(const obs_encoder_t *encoder)
{
	return encoder->context.name;
}
