/* Test-only oracle: the original libobs/obs-source-transition-start.c.h,
 * unmodified, with its fourteen functions renamed to oracle_* so they can
 * link next to the Rust shims. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_transition_start_refused oracle_obs_source_transition_start_refused
#define obs_source_transition_start_walks oracle_obs_source_transition_start_walks
#define obs_source_transition_start_admit oracle_obs_source_transition_start_admit
#define obs_source_transition_start_duration_ns oracle_obs_source_transition_start_duration_ns
#define obs_source_transition_start_restart oracle_obs_source_transition_start_restart
#define obs_source_transition_start_adds oracle_obs_source_transition_start_adds
#define obs_source_transition_start_removes oracle_obs_source_transition_start_removes
#define obs_source_transition_start_calls_back oracle_obs_source_transition_start_calls_back
#define obs_source_transition_start_unsized oracle_obs_source_transition_start_unsized
#define obs_source_transition_start_null_dest oracle_obs_source_transition_start_null_dest
#define obs_source_transition_start_activates oracle_obs_source_transition_start_activates
#define obs_source_transition_start_active_index oracle_obs_source_transition_start_active_index
#define obs_source_transition_start_manual_reset oracle_obs_source_transition_start_manual_reset
#define obs_source_transition_start_child_active oracle_obs_source_transition_start_child_active

#include "obs-source-transition-start.c.h"

#undef obs_source_transition_start_child_active
#undef obs_source_transition_start_manual_reset
#undef obs_source_transition_start_active_index
#undef obs_source_transition_start_activates
#undef obs_source_transition_start_null_dest
#undef obs_source_transition_start_unsized
#undef obs_source_transition_start_calls_back
#undef obs_source_transition_start_removes
#undef obs_source_transition_start_adds
#undef obs_source_transition_start_restart
#undef obs_source_transition_start_duration_ns
#undef obs_source_transition_start_admit
#undef obs_source_transition_start_walks
#undef obs_source_transition_start_refused

/* The values and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/source_transition_start.rs; INT64_MIN past the
 * end. */
int64_t oracle_source_transition_start_value(size_t which);

int64_t oracle_source_transition_start_value(size_t which)
{
	const int64_t values[] = {
		OBS_SOURCE_TRANSITION_START_MODE_MANUAL,
		OBS_SOURCE_TRANSITION_START_GO,
		OBS_SOURCE_TRANSITION_START_REFUSE,
		OBS_SOURCE_TRANSITION_START_DONE,
		(int64_t)sizeof(enum obs_source_transition_start_admission),
		(int64_t)sizeof(bool),
		(int64_t)sizeof(size_t),
	};

	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
