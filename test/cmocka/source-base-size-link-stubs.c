/* Standalone step tests never access live base-size fields. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_source_base_size_fields obs_source_base_size_fields(const obs_source_t *source)
{
	(void)source;
	abort();
}
