/* Unrelated source-step tests may pull these shims from their Rust archive.
 * They must never call this adapter without a real source fixture. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_source_identity_getter_fields obs_source_identity_getter_fields(const obs_source_t *source)
{
	(void)source;
	abort();
}
