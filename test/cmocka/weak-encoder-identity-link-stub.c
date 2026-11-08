/* Unrelated Rust-static consumers never call this query. */
#include <stdlib.h>
#include "obs-internal.h"

struct obs_weak_encoder_identity_fields obs_weak_encoder_identity_fields(const obs_weak_encoder_t *weak)
{
	(void)weak;
	abort();
}
