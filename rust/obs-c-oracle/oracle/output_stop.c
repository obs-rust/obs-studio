/* Test-only oracle: the original libobs/obs-output-stop.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust
 * shims, plus its constants as the C sees them. Nothing in it divides, so no
 * input traps on any host. */
#include <stddef.h>

#define obs_output_stop_encoded oracle_obs_output_stop_encoded
#define obs_output_stop_delay_on oracle_obs_output_stop_delay_on
#define obs_output_stop_has_video oracle_obs_output_stop_has_video
#define obs_output_stop_has_audio oracle_obs_output_stop_has_audio
#define obs_output_stop_service oracle_obs_output_stop_service
#define obs_output_stop_end_delay oracle_obs_output_stop_end_delay
#define obs_output_stop_thread_failed oracle_obs_output_stop_thread_failed

#include "obs-output-stop.c.h"

#undef obs_output_stop_thread_failed
#undef obs_output_stop_end_delay
#undef obs_output_stop_service
#undef obs_output_stop_has_audio
#undef obs_output_stop_has_video
#undef obs_output_stop_delay_on
#undef obs_output_stop_encoded

/* In the order of rust/obs-c-oracle/src/output_stop.rs. */
enum {
	ORACLE_OUTPUT_STOP_CONSTANT_COUNT = 6,
};

long long oracle_output_stop_constant(size_t which);

long long oracle_output_stop_constant(size_t which)
{
	const long long values[ORACLE_OUTPUT_STOP_CONSTANT_COUNT] = {
		OBS_OUTPUT_STOP_VIDEO,   OBS_OUTPUT_STOP_AUDIO,   OBS_OUTPUT_STOP_ENCODED,
		OBS_OUTPUT_STOP_SERVICE, (long long)sizeof(long), (long long)sizeof(int),
	};
	return which < ORACLE_OUTPUT_STOP_CONSTANT_COUNT ? values[which] : -1;
}
