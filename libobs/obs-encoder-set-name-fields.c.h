#include "libobs/build/libobs.pch.h"

struct obs_encoder_set_name_fields obs_encoder_set_name_fields(obs_encoder_t *encoder)
{
	return (struct obs_encoder_set_name_fields){&encoder->context, encoder->context.name};
}
