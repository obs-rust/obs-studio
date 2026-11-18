#include "libobs/build/libobs.pch.h"

bool obs_weak_source_expired(obs_weak_source_t *weak)
{
	return weak ? obs_weak_ref_expired(&weak->ref) : true;
}

bool obs_weak_source_references_source(obs_weak_source_t *weak, obs_source_t *source)
{
	return weak && source && weak->source == source;
}
