#include "libobs/build/libobs.pch.h"

bool obs_weak_object_references_object(obs_weak_object_t *weak, obs_object_t *object)
{
	return weak && object && weak->object == object;
}
