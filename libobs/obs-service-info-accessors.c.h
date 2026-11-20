/* Hidden service info-table and plugin-data accessors, extracted verbatim
 * for #3840. Declared in obs-internal.h. */
#include "libobs/build/libobs.pch.h"

void obs_service_set_info(obs_service_t *service, const struct obs_service_info *info)
{
	service->info = *info;
}

void obs_service_set_plugin_data(obs_service_t *service, void *data)
{
	service->context.data = data;
}

/* For the getters in obs-service-getters.c, which no longer see struct
 * obs_service: the plugin's callback table this service was created with.
 * Hidden; the Rust port calls it and obs_service_get_plugin_data() too
 * (#1899). */
const struct obs_service_info *obs_service_get_info_table(const obs_service_t *service)
{
	return &service->info;
}

/* The plugin data that create() returned, for the same getters. Hidden. */
void *obs_service_get_plugin_data(const obs_service_t *service)
{
	return service->context.data;
}
