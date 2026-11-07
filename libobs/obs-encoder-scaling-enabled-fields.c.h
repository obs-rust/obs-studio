#include "libobs/build/libobs.pch.h"

struct obs_encoder_scaling_enabled_fields obs_encoder_scaling_enabled_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_scaling_enabled_fields){encoder ? &encoder->scaled_width : NULL,
							   encoder ? &encoder->scaled_height : NULL};
}
