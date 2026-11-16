/* Real private layout for standalone Rust-static consumers. */
#include "obs-internal.h"

const char *obs_source_name_field(const obs_source_t *source);
const char *obs_source_name_field(const obs_source_t *source)
{
	return source->context.name;
}
