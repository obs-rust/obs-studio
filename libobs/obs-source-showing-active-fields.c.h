#include "libobs/build/libobs.pch.h"

const volatile long *obs_source_activate_refs_field(const obs_source_t *source);
const volatile long *obs_source_activate_refs_field(const obs_source_t *source)
{
	return &source->activate_refs;
}

const volatile long *obs_source_show_refs_field(const obs_source_t *source);
const volatile long *obs_source_show_refs_field(const obs_source_t *source)
{
	return &source->show_refs;
}
