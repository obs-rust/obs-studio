#include "libobs/build/libobs.pch.h"

/* The context fields the settings and identity functions in
 * obs-service-getters.c read, for them and their Rust port (#1913). Hidden. */
const char *obs_service_get_context_name(const obs_service_t *service)
{
	return service->context.name;
}

obs_data_t *obs_service_get_context_settings(const obs_service_t *service)
{
	return service->context.settings;
}
