#include "libobs/build/libobs.pch.h"

struct obs_encoder_preferred_color_setters_fields obs_encoder_preferred_color_setters_fields(obs_encoder_t *encoder)
{
	return (struct obs_encoder_preferred_color_setters_fields){&encoder->info.type, &encoder->preferred_space,
								   &encoder->preferred_range};
}
