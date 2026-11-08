/* Unrelated standalone Rust tests do not own output fixtures. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_output_service_getter_fields obs_output_service_getter_fields(const obs_output_t *output)
{
	(void)output;
	abort();
}
