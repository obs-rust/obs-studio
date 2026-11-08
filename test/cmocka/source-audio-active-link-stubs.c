/* Unrelated source-step archive consumers never call this query. */
#include <stdlib.h>
#include "obs-internal.h"

const bool *obs_source_audio_active_field(const obs_source_t *source)
{
	(void)source;
	abort();
}
