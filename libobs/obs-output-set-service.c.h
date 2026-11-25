#include "libobs/build/libobs.pch.h"

void obs_output_set_service(obs_output_t *output, obs_service_t *service)
{
	if (!obs_output_valid(output, "obs_output_set_service"))
		return;
	if (!log_flag_service(output, __func__) || active(output) || !service || service->active)
		return;

	if (service->output)
		service->output->service = NULL;

	output->service = service;
	service->output = output;
}
