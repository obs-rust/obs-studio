#include "libobs/build/libobs.pch.h"

long *obs_source_async_rotation_fields(obs_source_t *source)
{
	return &source->async_rotation;
}
