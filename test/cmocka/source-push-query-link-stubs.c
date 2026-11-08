/* Unrelated static source-step consumers must never call this private adapter. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_source_push_query_fields obs_source_push_query_fields(obs_source_t *source)
{
	(void)source;
	abort();
}
