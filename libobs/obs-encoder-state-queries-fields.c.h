#include "libobs/build/libobs.pch.h"

struct obs_encoder_state_queries_fields obs_encoder_state_queries_fields(const obs_encoder_t *encoder)
{
	return (struct obs_encoder_state_queries_fields){&encoder->active, &encoder->paused};
}
