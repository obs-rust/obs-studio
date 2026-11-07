#include "libobs/build/libobs.pch.h"

#define get_weak(service) ((obs_weak_service_t *)service->context.control)

void obs_service_set_destroy(obs_service_t *service)
{
	service->destroy = true;
}

bool obs_service_is_destroying(const obs_service_t *service)
{
	return service->destroy;
}

bool obs_service_is_active(const obs_service_t *service)
{
	return service->active;
}

void obs_service_set_active(obs_service_t *service, bool active)
{
	service->active = active;
}

bool obs_service_has_output(const obs_service_t *service)
{
	return service->output != NULL;
}

void obs_service_clear_output(obs_service_t *service)
{
	service->output = NULL;
}

/* Only called with an output. */
void obs_service_detach_from_output(obs_service_t *service)
{
	service->output->service = NULL;
}

obs_weak_service_t *obs_service_get_control(const obs_service_t *service)
{
	return get_weak(service);
}

#undef get_weak
