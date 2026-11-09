#include "libobs/build/libobs.pch.h"

void obs_weak_object_addref(obs_weak_object_t *weak)
{
	if (!weak)
		return;

	obs_weak_ref_addref(&weak->ref);
}

void obs_weak_object_release(obs_weak_object_t *weak)
{
	if (!weak)
		return;

	if (obs_weak_ref_release(&weak->ref))
		bfree(weak);
}

obs_weak_object_t *obs_object_get_weak_object(obs_object_t *object)
{
	if (!object)
		return NULL;

	obs_weak_object_t *weak = object->control;
	obs_weak_object_addref(weak);
	return weak;
}

obs_object_t *obs_weak_object_get_object(obs_weak_object_t *weak)
{
	if (!weak)
		return NULL;

	if (obs_weak_ref_get_ref(&weak->ref))
		return weak->object;

	return NULL;
}
