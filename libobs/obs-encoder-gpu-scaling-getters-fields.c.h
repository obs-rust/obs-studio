#include "libobs/build/libobs.pch.h"

struct obs_encoder_gpu_scaling_getters_fields obs_encoder_gpu_scaling_getters_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_gpu_scaling_getters_fields){encoder ? &encoder->info.type : NULL,
							       encoder ? &encoder->gpu_scale_type : NULL,
							       encoder ? &encoder->context.name : NULL};
}
