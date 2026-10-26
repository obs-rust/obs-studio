/* Test-only oracle: the original libobs/obs-source-audio-place.c.h,
 * unmodified, with its eleven functions renamed to oracle_* so they can
 * link next to the Rust shims. */
#include <stddef.h>
#include <stdint.h>

#define obs_source_audio_place_started oracle_obs_source_audio_place_started
#define obs_source_audio_place_older oracle_obs_source_audio_place_older
#define obs_source_audio_place_size oracle_obs_source_audio_place_size
#define obs_source_audio_place_offset oracle_obs_source_audio_place_offset
#define obs_source_audio_place_fits oracle_obs_source_audio_place_fits
#define obs_source_audio_place_pop_back oracle_obs_source_audio_place_pop_back
#define obs_source_audio_place_outputs oracle_obs_source_audio_place_outputs
#define obs_source_audio_place_action_frame oracle_obs_source_audio_place_action_frame
#define obs_source_audio_place_action_stops oracle_obs_source_audio_place_action_stops
#define obs_source_audio_place_fill_end oracle_obs_source_audio_place_fill_end
#define obs_source_audio_place_mix_on oracle_obs_source_audio_place_mix_on

#include "obs-source-audio-place.c.h"

#undef obs_source_audio_place_mix_on
#undef obs_source_audio_place_fill_end
#undef obs_source_audio_place_action_stops
#undef obs_source_audio_place_action_frame
#undef obs_source_audio_place_outputs
#undef obs_source_audio_place_pop_back
#undef obs_source_audio_place_fits
#undef obs_source_audio_place_offset
#undef obs_source_audio_place_size
#undef obs_source_audio_place_older
#undef obs_source_audio_place_started

/* The values and sizes the port relies on, in the order of
 * rust/obs-c-oracle/src/source_audio_place.rs; INT64_MIN past the end. */
int64_t oracle_source_audio_place_value(size_t which);

int64_t oracle_source_audio_place_value(size_t which)
{
	const int64_t values[] = {
		OBS_SOURCE_AUDIO_PLACE_FRAMES,
		(int64_t)OBS_SOURCE_AUDIO_PLACE_MAX_BUF_SIZE,
		OBS_SOURCE_AUDIO_PLACE_MONITOR_ONLY,
		(int64_t)sizeof(size_t),
	};

	return which < sizeof(values) / sizeof(values[0]) ? values[which] : INT64_MIN;
}
