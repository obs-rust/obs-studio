#include "libobs/build/libobs.pch.h"

struct obs_encoder_counter_fields obs_encoder_counter_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_counter_fields){encoder ? &encoder->encoded_frames : NULL,
						   encoder ? &encoder->pause.ts_offset : NULL};
}
