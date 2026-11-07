#include "libobs/build/libobs.pch.h"

bool obs_weak_service_weak_get_ref(obs_weak_service_t *weak)
{
	return obs_weak_ref_get_ref(&weak->ref);
}

obs_service_t *obs_weak_service_target(const obs_weak_service_t *weak)
{
	return weak->service;
}
