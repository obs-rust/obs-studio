/* Unrelated source-step tests pull these shims from the same Rust archive
 * member. They never call the getters; fail loudly if that changes. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_source_filter_relationship_fields obs_source_filter_relationship_fields(const obs_source_t *source)
{
	(void)source;
	abort();
}
