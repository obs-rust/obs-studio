/* Test-only oracle: the original libobs/obs-source-deinterlace-pacing.c.h,
 * unmodified, with its six functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <stddef.h>

#define obs_source_deinterlace_pacing_hold oracle_obs_source_deinterlace_pacing_hold
#define obs_source_deinterlace_pacing_wait_for_next oracle_obs_source_deinterlace_pacing_wait_for_next
#define obs_source_deinterlace_pacing_waiting_frame_ts oracle_obs_source_deinterlace_pacing_waiting_frame_ts
#define obs_source_deinterlace_pacing_half_duration oracle_obs_source_deinterlace_pacing_half_duration
#define obs_source_deinterlace_pacing_buffered_offset oracle_obs_source_deinterlace_pacing_buffered_offset
#define obs_source_deinterlace_pacing_reanchor oracle_obs_source_deinterlace_pacing_reanchor

#include "obs-source-deinterlace-pacing.c.h"

#undef obs_source_deinterlace_pacing_reanchor
#undef obs_source_deinterlace_pacing_buffered_offset
#undef obs_source_deinterlace_pacing_half_duration
#undef obs_source_deinterlace_pacing_waiting_frame_ts
#undef obs_source_deinterlace_pacing_wait_for_next
#undef obs_source_deinterlace_pacing_hold

/* The constants of the header, in the order of
 * rust/obs-c-oracle/src/source_deinterlace_pacing.rs. */
enum {
	ORACLE_SOURCE_DEINTERLACE_PACING_CONSTANT_COUNT = 3,
};

unsigned long long oracle_source_deinterlace_pacing_constant(size_t which);

unsigned long long oracle_source_deinterlace_pacing_constant(size_t which)
{
	const unsigned long long values[ORACLE_SOURCE_DEINTERLACE_PACING_CONSTANT_COUNT] = {
		OBS_SOURCE_DEINTERLACE_PACING_FRAME_TOLERANCE,
		OBS_SOURCE_DEINTERLACE_PACING_TWOX_TOLERANCE,
		OBS_SOURCE_DEINTERLACE_PACING_TS_JUMP_THRESHOLD,
	};
	return which < ORACLE_SOURCE_DEINTERLACE_PACING_CONSTANT_COUNT ? values[which] : 0;
}
