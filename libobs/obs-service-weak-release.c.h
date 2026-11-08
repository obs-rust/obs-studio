#include "libobs/build/libobs-ported-service-getters.pch.h"

bool obs_weak_service_strong_release(obs_weak_service_t *weak)
{
	return obs_ref_release(&weak->ref);
}

void obs_weak_service_weak_addref(obs_weak_service_t *weak)
{
	obs_weak_ref_addref(&weak->ref);
}

bool obs_weak_service_weak_release(obs_weak_service_t *weak)
{
	return obs_weak_ref_release(&weak->ref);
}

void obs_weak_service_free(obs_weak_service_t *weak)
{
	bfree(weak);
}
