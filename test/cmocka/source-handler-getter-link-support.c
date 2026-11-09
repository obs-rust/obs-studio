/* Live source fields for standalone Rust-static consumers, including real objects. */
#include "obs-internal.h"

struct obs_source_handler_getters_fields obs_source_handler_getters_fields(const obs_source_t *source)
{
	return (struct obs_source_handler_getters_fields){&source->context.procs};
}
