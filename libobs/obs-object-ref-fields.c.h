#include "libobs/build/libobs.pch.h"

obs_weak_object_t *obs_object_ref_control(const obs_object_t *object)
{
	return object->control;
}

void obs_object_ref_destroy(obs_object_t *object)
{
	object->destroy(object);
}
