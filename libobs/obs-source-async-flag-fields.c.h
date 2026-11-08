#include "libobs/build/libobs.pch.h"

struct obs_source_async_flag_fields obs_source_async_flag_fields(const obs_source_t *source)
{
	return (struct obs_source_async_flag_fields){source ? &source->async_unbuffered : NULL,
						     source ? &source->async_decoupled : NULL};
}
