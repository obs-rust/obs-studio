#include "libobs/build/libobs.pch.h"

struct obs_encoder_preferred_color_space_fields obs_encoder_preferred_color_space_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_preferred_color_space_fields){encoder ? &encoder->info.type : NULL,
								 encoder ? &encoder->preferred_space : NULL};
}
