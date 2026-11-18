/* Hidden borrowed service handlers, extracted verbatim for #3840. */
#include "libobs/build/libobs.pch.h"

signal_handler_t *obs_service_get_signal_handler(const obs_service_t *service);
signal_handler_t *obs_service_get_signal_handler(const obs_service_t *service)
{
	return obs_service_valid(service, "obs_service_get_signal_handler") ? service->context.signals : NULL;
}

proc_handler_t *obs_service_get_proc_handler(const obs_service_t *service);
proc_handler_t *obs_service_get_proc_handler(const obs_service_t *service)
{
	return obs_service_valid(service, "obs_service_get_proc_handler") ? service->context.procs : NULL;
}
