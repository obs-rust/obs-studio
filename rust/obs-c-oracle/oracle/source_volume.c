/* Test-only oracle: the original libobs/obs-source-volume.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust
 * shims. */
#define obs_source_volume_snap oracle_obs_source_volume_snap
#define obs_source_volume_action_due oracle_obs_source_volume_action_due
#define obs_source_volume_step oracle_obs_source_volume_step
#define obs_source_volume_mixes oracle_obs_source_volume_mixes

#include "obs-source-volume.c.h"

#undef obs_source_volume_snap
#undef obs_source_volume_action_due
#undef obs_source_volume_step
#undef obs_source_volume_mixes

/* The header's enum values and size, for the Rust side. */
unsigned oracle_source_volume_step_value(int which);
unsigned oracle_source_volume_step_size(void);

unsigned oracle_source_volume_step_value(int which)
{
	switch (which) {
	case 0:
		return OBS_SOURCE_VOLUME_SKIP;
	case 1:
		return OBS_SOURCE_VOLUME_CLEAR;
	default:
		return OBS_SOURCE_VOLUME_MULTIPLY;
	}
}

unsigned oracle_source_volume_step_size(void)
{
	return (unsigned)sizeof(enum obs_source_volume_op);
}
