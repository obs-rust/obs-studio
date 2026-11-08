/* Unrelated standalone Rust tests do not own output fixtures. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_output_handler_getters_fields obs_output_handler_getters_fields(const obs_output_t *output)
{
	(void)output;
	abort();
}
