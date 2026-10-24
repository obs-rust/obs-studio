/* Test-only oracle: the original libobs/obs-source-transition-clock.c.h,
 * unmodified, with its seven functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <float.h>
#include <stddef.h>

#define obs_source_transition_clock_time oracle_obs_source_transition_clock_time
#define obs_source_transition_clock_manual oracle_obs_source_transition_clock_manual
#define obs_source_transition_clock_size oracle_obs_source_transition_clock_size
#define obs_source_transition_clock_fold oracle_obs_source_transition_clock_fold
#define obs_source_transition_clock_fit oracle_obs_source_transition_clock_fit
#define obs_source_transition_clock_mix_spaces oracle_obs_source_transition_clock_mix_spaces
#define obs_source_transition_clock_min_ts oracle_obs_source_transition_clock_min_ts

#include "obs-source-transition-clock.c.h"

#undef obs_source_transition_clock_min_ts
#undef obs_source_transition_clock_mix_spaces
#undef obs_source_transition_clock_fit
#undef obs_source_transition_clock_fold
#undef obs_source_transition_clock_size
#undef obs_source_transition_clock_manual
#undef obs_source_transition_clock_time

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/source_transition_clock.rs. LDBL_MANT_DIG is the
 * significand of the long double calc_time() divides in: 64 for x87, 53
 * where it is a double, 113 for IEEE quad. */
enum {
	ORACLE_SOURCE_TRANSITION_CLOCK_LAYOUT_COUNT = 17,
};

size_t oracle_source_transition_clock_layout(size_t which);

size_t oracle_source_transition_clock_layout(size_t which)
{
	const size_t values[ORACLE_SOURCE_TRANSITION_CLOCK_LAYOUT_COUNT] = {
		OBS_SOURCE_TRANSITION_CLOCK_MODE_MANUAL,
		OBS_SOURCE_TRANSITION_CLOCK_SCALE_MAX_ONLY,
		OBS_SOURCE_TRANSITION_CLOCK_SCALE_ASPECT,
		OBS_SOURCE_TRANSITION_CLOCK_SCALE_STRETCH,
		OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB,
		OBS_SOURCE_TRANSITION_CLOCK_CS_SRGB_16F,
		OBS_SOURCE_TRANSITION_CLOCK_CS_709_EXTENDED,
		OBS_SOURCE_TRANSITION_CLOCK_CS_709_SCRGB,
		sizeof(enum obs_source_transition_clock_step),
		OBS_SOURCE_TRANSITION_CLOCK_HOLD,
		OBS_SOURCE_TRANSITION_CLOCK_SNAP,
		OBS_SOURCE_TRANSITION_CLOCK_STEP,
		sizeof(int),
		sizeof(bool),
		sizeof(float),
		LDBL_MANT_DIG,
		FLT_EVAL_METHOD < 0 ? 99 : (size_t)FLT_EVAL_METHOD,
	};
	return which < ORACLE_SOURCE_TRANSITION_CLOCK_LAYOUT_COUNT ? values[which] : 0;
}
