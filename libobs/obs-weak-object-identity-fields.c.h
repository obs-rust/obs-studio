#include "libobs/build/libobs.pch.h"

struct obs_weak_object_identity_fields obs_weak_object_identity_fields(const obs_weak_object_t *weak)
{
	return (struct obs_weak_object_identity_fields){&weak->object};
}
