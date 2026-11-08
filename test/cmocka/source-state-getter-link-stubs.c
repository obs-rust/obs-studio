/* Unrelated source-step tests can pull these shims from the same Rust archive
 * member. They never call the getters; fail loudly if that contract changes. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_source_state_getter_fields obs_source_state_getter_fields(const obs_source_t *source)
{
	(void)source;
	abort();
}
