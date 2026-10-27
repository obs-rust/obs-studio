/* Test-only oracle: the original libobs/obs-source-transition-load.c.h,
 * unmodified, with its six functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_transition_load_is_transition oracle_obs_source_transition_load_is_transition
#define obs_source_transition_load_scale_type oracle_obs_source_transition_load_scale_type
#define obs_source_transition_load_looks_up oracle_obs_source_transition_load_looks_up
#define obs_source_transition_load_source oracle_obs_source_transition_load_source
#define obs_source_transition_load_swaps oracle_obs_source_transition_load_swaps
#define obs_source_transition_load_copy_stores oracle_obs_source_transition_load_copy_stores

#include "obs-source-transition-load.c.h"

#undef obs_source_transition_load_copy_stores
#undef obs_source_transition_load_swaps
#undef obs_source_transition_load_source
#undef obs_source_transition_load_looks_up
#undef obs_source_transition_load_scale_type
#undef obs_source_transition_load_is_transition

/* The values and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/source_transition_load.rs; INT64_MIN past the
 * end. */
int64_t oracle_source_transition_load_value(size_t which);

int64_t oracle_source_transition_load_value(size_t which)
{
	const int64_t values[] = {
		OBS_SOURCE_TRANSITION_LOAD_TYPE_TRANSITION,
		OBS_SOURCE_TRANSITION_LOAD_SCALE_MAX_ONLY,
		OBS_SOURCE_TRANSITION_LOAD_SCALE_ASPECT,
		OBS_SOURCE_TRANSITION_LOAD_SCALE_STRETCH,
		OBS_SOURCE_TRANSITION_LOAD_KEEP,
		OBS_SOURCE_TRANSITION_LOAD_NOT_FOUND,
		OBS_SOURCE_TRANSITION_LOAD_REFUSE,
		(int64_t)sizeof(enum obs_source_transition_load_outcome),
		(int64_t)sizeof(bool),
		(int64_t)sizeof(int),
	};

	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
