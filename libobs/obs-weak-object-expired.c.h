#include "libobs/build/libobs.pch.h"

bool obs_weak_object_expired(obs_weak_object_t *weak)
{
	return weak ? obs_weak_ref_expired(&weak->ref) : true;
}
