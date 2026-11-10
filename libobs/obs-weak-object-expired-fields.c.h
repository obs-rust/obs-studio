#include "libobs/build/libobs.pch.h"

const volatile long *obs_weak_object_expired_field(const obs_weak_object_t *weak)
{
	return &weak->ref.refs;
}
