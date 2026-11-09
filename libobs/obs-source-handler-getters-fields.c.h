#include "libobs/build/libobs.pch.h"

struct obs_source_handler_getters_fields obs_source_handler_getters_fields(const obs_source_t *source)
{
	return (struct obs_source_handler_getters_fields){&source->context.procs};
}
