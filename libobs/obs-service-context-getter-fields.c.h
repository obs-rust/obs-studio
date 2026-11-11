#include "libobs/build/libobs.pch.h"

char *const *obs_service_context_name_field(const obs_service_t *service);
obs_data_t *const *obs_service_context_settings_field(const obs_service_t *service);

char *const *obs_service_context_name_field(const obs_service_t *service)
{
	return &service->context.name;
}

obs_data_t *const *obs_service_context_settings_field(const obs_service_t *service)
{
	return &service->context.settings;
}
