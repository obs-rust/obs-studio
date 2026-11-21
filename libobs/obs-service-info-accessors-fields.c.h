#include "libobs/build/libobs.pch.h"

/* Private field addresses for the Rust service info-table and plugin-data
 * accessors (#3840): the service's own copy of its plugin table and the
 * plugin data create() returned. The service is never NULL here, as in the
 * original; no copy, lock or reference is taken. */
struct obs_service_info_accessors_fields obs_service_info_accessors_fields(obs_service_t *service)
{
	return (struct obs_service_info_accessors_fields){&service->info, &service->context.data};
}
