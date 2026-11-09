#include "libobs/build/libobs.pch.h"

proc_handler_t *obs_source_get_proc_handler(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_get_proc_handler") ? source->context.procs : NULL;
}
