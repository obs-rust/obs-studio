#include "libobs/build/libobs.pch.h"

uint32_t *obs_source_default_flags_field(obs_source_t *source);
uint32_t *obs_source_default_flags_field(obs_source_t *source)
{
	return &source->default_flags;
}
