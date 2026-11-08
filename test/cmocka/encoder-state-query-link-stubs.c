/* Unrelated step tests can pull these shims from the same Rust archive
 * member. They never call the queries; fail loudly if that changes. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_encoder_state_queries_fields obs_encoder_state_queries_fields(const obs_encoder_t *encoder)
{
	(void)encoder;
	abort();
}
