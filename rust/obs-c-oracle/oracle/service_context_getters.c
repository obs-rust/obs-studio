/* Independent original C getters; field addresses use real private layouts. */
#define obs_service_get_context_name oracle_context_name
#define obs_service_get_context_settings oracle_context_settings
#include "libobs/obs-service-context-getters.c.h"
#include "service-context-getter-fields.inc"

bool oracle_context_field_addresses(const obs_service_t *service);
bool oracle_context_field_addresses(const obs_service_t *service)
{
	return obs_service_context_name_field(service) == &service->context.name &&
	       obs_service_context_settings_field(service) == &service->context.settings;
}

void *oracle_context_snapshot(const obs_service_t *service);
void *oracle_context_snapshot(const obs_service_t *service)
{
	void *copy = malloc(sizeof(*service));
	memcpy(copy, service, sizeof(*service));
	return copy;
}

bool oracle_context_unchanged(const obs_service_t *service, void *copy);
bool oracle_context_unchanged(const obs_service_t *service, void *copy)
{
	bool same = memcmp(service, copy, sizeof(*service)) == 0;
	free(copy);
	return same;
}
