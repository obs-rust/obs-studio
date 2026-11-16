#include "libobs/build/libobs.pch.h"

struct obs_encoder_preferred_range_fields obs_encoder_preferred_range_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_preferred_range_fields){encoder ? &encoder->info.type : NULL,
							   encoder ? &encoder->preferred_range : NULL};
}
