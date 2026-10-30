/* Test-only oracle: the original libobs/obs-output-pause.c.h, unmodified,
 * with its eight functions renamed to oracle_* so they can link next to the
 * Rust shims. The parity test never passes the input where C is undefined
 * (a frame interval of 0, which obs_output_pause_closest_ts() divides by). */
#include <stdbool.h>
#include <stdint.h>

#define obs_output_pause_allowed oracle_obs_output_pause_allowed
#define obs_output_pause_already oracle_obs_output_pause_already
#define obs_output_pause_lock_set oracle_obs_output_pause_lock_set
#define obs_output_pause_closest_ts oracle_obs_output_pause_closest_ts
#define obs_output_pause_can_start oracle_obs_output_pause_can_start
#define obs_output_pause_can_stop oracle_obs_output_pause_can_stop
#define obs_output_pause_ends oracle_obs_output_pause_ends
#define obs_output_pause_offset oracle_obs_output_pause_offset

#include "obs-output-pause.c.h"

#undef obs_output_pause_offset
#undef obs_output_pause_ends
#undef obs_output_pause_can_stop
#undef obs_output_pause_can_start
#undef obs_output_pause_closest_ts
#undef obs_output_pause_lock_set
#undef obs_output_pause_already
#undef obs_output_pause_allowed
