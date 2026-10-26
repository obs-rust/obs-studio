/* Test-only oracle: the original libobs/obs-output-start.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust
 * shims, plus its constants as the C sees them. Nothing in it divides, so no
 * input traps on any host. */
#include <stddef.h>

#define obs_output_start_service oracle_obs_output_start_service
#define obs_output_start_drop_restart_ref oracle_obs_output_start_drop_restart_ref
#define obs_output_start_init_video oracle_obs_output_start_init_video
#define obs_output_start_init_audio oracle_obs_output_start_init_audio
#define obs_output_start_reset_raw oracle_obs_output_start_reset_raw
#define obs_output_start_pair oracle_obs_output_start_pair

#include "obs-output-start.c.h"

#undef obs_output_start_pair
#undef obs_output_start_reset_raw
#undef obs_output_start_init_audio
#undef obs_output_start_init_video
#undef obs_output_start_drop_restart_ref
#undef obs_output_start_service

/* In the order of rust/obs-c-oracle/src/output_start.rs. */
enum {
	ORACLE_OUTPUT_START_CONSTANT_COUNT = 5,
};

long long oracle_output_start_constant(size_t which);

long long oracle_output_start_constant(size_t which)
{
	const long long values[ORACLE_OUTPUT_START_CONSTANT_COUNT] = {
		OBS_OUTPUT_START_VIDEO,   OBS_OUTPUT_START_AUDIO,  OBS_OUTPUT_START_ENCODED,
		OBS_OUTPUT_START_SERVICE, (long long)sizeof(long),
	};
	return which < ORACLE_OUTPUT_START_CONSTANT_COUNT ? values[which] : -1;
}
