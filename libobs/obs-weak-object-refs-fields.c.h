#include "libobs/build/libobs.pch.h"

struct obs_weak_object_refs_fields obs_weak_object_refs_fields(obs_weak_object_t *weak)
{
	return (struct obs_weak_object_refs_fields){&weak->ref.refs, &weak->ref.weak_refs, weak->object};
}

obs_weak_object_t *obs_object_refs_control(obs_object_t *object)
{
	return object->control;
}
