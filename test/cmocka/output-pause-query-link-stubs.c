/* Pure-rule archive consumers never call these output queries. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_output_pause_queries_fields obs_output_pause_queries_fields(obs_output_t *output)
{
	(void)output;
	abort();
}

void obs_output_pause_queries_lock(void *mutex)
{
	(void)mutex;
	abort();
}

void obs_output_pause_queries_unlock(void *mutex)
{
	(void)mutex;
	abort();
}
