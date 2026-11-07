#include "libobs/build/libobs.pch.h"

struct obs_encoder_frame_rate_divisor_fields obs_encoder_frame_rate_divisor_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_frame_rate_divisor_fields){encoder ? &encoder->info.type : NULL,
							      encoder ? &encoder->frame_rate_divisor : NULL,
							      encoder ? &encoder->context.name : NULL};
}
