#include "libobs/build/libobs.pch.h"

struct obs_encoder_dimensions_fields obs_encoder_dimensions_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_dimensions_fields){encoder ? &encoder->info.type : NULL,
						      encoder ? &encoder->media : NULL,
						      encoder ? &encoder->scaled_width : NULL,
						      encoder ? &encoder->scaled_height : NULL,
						      encoder ? &encoder->context.name : NULL};
}
