#include "libobs/build/libobs.pch.h"

struct obs_service_handler_getters_fields obs_service_handler_getters_fields(const obs_service_t *service)
{
	return (struct obs_service_handler_getters_fields){&service->context.signals, &service->context.procs};
}
