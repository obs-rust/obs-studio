/* Test-only oracle: the original libobs/obs-encoder-lifecycle.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust
 * shims, plus its constants as the C sees them. Nothing in it divides, so no
 * input traps on any host. */
#include <stddef.h>

#define obs_encoder_lifecycle_first oracle_obs_encoder_lifecycle_first
#define obs_encoder_lifecycle_found oracle_obs_encoder_lifecycle_found
#define obs_encoder_lifecycle_last oracle_obs_encoder_lifecycle_last
#define obs_encoder_lifecycle_pass_texture oracle_obs_encoder_lifecycle_pass_texture
#define obs_encoder_lifecycle_lcm_unset oracle_obs_encoder_lifecycle_lcm_unset
#define obs_encoder_lifecycle_started_inc oracle_obs_encoder_lifecycle_started_inc
#define obs_encoder_lifecycle_group_ready oracle_obs_encoder_lifecycle_group_ready
#define obs_encoder_lifecycle_started_dec oracle_obs_encoder_lifecycle_started_dec
#define obs_encoder_lifecycle_group_idle oracle_obs_encoder_lifecycle_group_idle
#define obs_encoder_lifecycle_prepend_sei oracle_obs_encoder_lifecycle_prepend_sei

#include "obs-encoder-lifecycle.c.h"

#undef obs_encoder_lifecycle_prepend_sei
#undef obs_encoder_lifecycle_group_idle
#undef obs_encoder_lifecycle_started_dec
#undef obs_encoder_lifecycle_group_ready
#undef obs_encoder_lifecycle_started_inc
#undef obs_encoder_lifecycle_lcm_unset
#undef obs_encoder_lifecycle_pass_texture
#undef obs_encoder_lifecycle_last
#undef obs_encoder_lifecycle_found
#undef obs_encoder_lifecycle_first

/* In the order of rust/obs-c-oracle/src/encoder_lifecycle.rs. */
enum {
	ORACLE_ENCODER_LIFECYCLE_CONSTANT_COUNT = 2,
};

unsigned long long oracle_encoder_lifecycle_constant(size_t which, int *ok);

unsigned long long oracle_encoder_lifecycle_constant(size_t which, int *ok)
{
	const unsigned long long values[ORACLE_ENCODER_LIFECYCLE_CONSTANT_COUNT] = {
		OBS_ENCODER_LIFECYCLE_CAP_PASS_TEXTURE,
		OBS_ENCODER_LIFECYCLE_NOT_FOUND,
	};
	*ok = which < ORACLE_ENCODER_LIFECYCLE_CONSTANT_COUNT;
	return *ok ? values[which] : 0;
}
