/* Original acquisition bodies with private names, over the real C layout. */
#include <stddef.h>

#define obs_weak_service_weak_get_ref oracle_obs_weak_service_weak_get_ref
#define obs_weak_service_target oracle_obs_weak_service_target
#include "obs-service-weak-acquire.c.h"
#undef obs_weak_service_target
#undef obs_weak_service_weak_get_ref

void oracle_service_weak_acquire_layout(size_t *layout);
void oracle_service_weak_acquire_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_weak_ref);
	layout[1] = _Alignof(struct obs_weak_ref);
	layout[2] = offsetof(struct obs_weak_ref, refs);
	layout[3] = offsetof(struct obs_weak_ref, weak_refs);
	layout[4] = sizeof(struct obs_weak_service);
	layout[5] = _Alignof(struct obs_weak_service);
	layout[6] = offsetof(struct obs_weak_service, ref);
	layout[7] = offsetof(struct obs_weak_service, service);
}
