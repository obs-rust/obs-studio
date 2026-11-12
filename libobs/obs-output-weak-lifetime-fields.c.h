#include "libobs/build/libobs.pch.h"

obs_weak_output_t *obs_output_weak_lifetime_control(obs_output_t *output)
{
	return (obs_weak_output_t *)output->context.control;
}

void obs_output_weak_lifetime_addref(obs_weak_output_t *weak)
{
	obs_weak_ref_addref(&weak->ref);
}

bool obs_output_weak_lifetime_release(obs_weak_output_t *weak)
{
	return obs_weak_ref_release(&weak->ref);
}

bool obs_output_weak_lifetime_promote(obs_weak_output_t *weak)
{
	return obs_weak_ref_get_ref(&weak->ref);
}
