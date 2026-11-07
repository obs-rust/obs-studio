#include "libobs/build/libobs.pch.h"

struct obs_service_state_fields obs_service_state_fields(const obs_service_t *service)
{
	struct obs_service *s = (struct obs_service *)service;
	return (struct obs_service_state_fields){&s->active, &s->destroy, &s->output, &s->context.control};
}

obs_service_t **obs_service_output_service_field(obs_output_t *output)
{
	return &output->service;
}
