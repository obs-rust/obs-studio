#include "libobs/build/libobs.pch.h"

const volatile long *obs_source_weak_predicates_refs_field(const obs_weak_source_t *weak);
const volatile long *obs_source_weak_predicates_refs_field(const obs_weak_source_t *weak)
{
	return &weak->ref.refs;
}

const obs_source_t *obs_source_weak_predicates_source_field(const obs_weak_source_t *weak);
const obs_source_t *obs_source_weak_predicates_source_field(const obs_weak_source_t *weak)
{
	return weak->source;
}
