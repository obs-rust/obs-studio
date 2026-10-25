/* Test-only oracle: the original libobs/obs-audio-min-ts.c.h, unmodified,
 * with its three functions renamed to oracle_* so they can link next to the
 * Rust shims. util_mul_div64 stays the header inline. */
#include <stdbool.h>
#include <stddef.h>

#define obs_audio_min_ts_earlier oracle_obs_audio_min_ts_earlier
#define obs_audio_min_ts_insufficient oracle_obs_audio_min_ts_insufficient
#define obs_audio_min_ts_buffering oracle_obs_audio_min_ts_buffering

#include "obs-audio-min-ts.c.h"

#undef obs_audio_min_ts_buffering
#undef obs_audio_min_ts_insufficient
#undef obs_audio_min_ts_earlier

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/audio_min_ts.rs. */
enum {
	ORACLE_AUDIO_MIN_TS_LAYOUT_COUNT = 4,
};

size_t oracle_audio_min_ts_layout(size_t which);

size_t oracle_audio_min_ts_layout(size_t which)
{
	const size_t values[ORACLE_AUDIO_MIN_TS_LAYOUT_COUNT] = {
		sizeof(enum obs_audio_buffering_choice),
		OBS_AUDIO_BUFFERING_NONE,
		OBS_AUDIO_BUFFERING_FIXED,
		OBS_AUDIO_BUFFERING_ADD,
	};
	return which < ORACLE_AUDIO_MIN_TS_LAYOUT_COUNT ? values[which] : 0;
}
