#include "libobs/build/libobs.pch.h"

volatile bool *obs_source_async_compensation_field(obs_source_t *source)
{
	return &source->async_compensation;
}
