/* Test-only oracle: the original libobs/obs-output-slots.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust
 * shims, plus its constants as the C sees them. Nothing in it divides or
 * calls out. obs_output_slots_mixer_mask() shifts by mixer_idx: callers keep
 * it below the width of size_t, where the C is defined. */
#include <stddef.h>

#define obs_output_slots_first_video oracle_obs_output_slots_first_video
#define obs_output_slots_first_audio oracle_obs_output_slots_first_audio
#define obs_output_slots_mixer_in_range oracle_obs_output_slots_mixer_in_range
#define obs_output_slots_mixer_mask oracle_obs_output_slots_mixer_mask
#define obs_output_slots_video_type_ok oracle_obs_output_slots_video_type_ok
#define obs_output_slots_audio_type_ok oracle_obs_output_slots_audio_type_ok
#define obs_output_slots_video_index_ok oracle_obs_output_slots_video_index_ok
#define obs_output_slots_audio_index_ok oracle_obs_output_slots_audio_index_ok
#define obs_output_slots_default_index oracle_obs_output_slots_default_index
#define obs_output_slots_scaled_axis_set oracle_obs_output_slots_scaled_axis_set
#define obs_output_slots_video_slot oracle_obs_output_slots_video_slot
#define obs_output_slots_audio_slot oracle_obs_output_slots_audio_slot

#include "obs-output-slots.c.h"

#undef obs_output_slots_audio_slot
#undef obs_output_slots_video_slot
#undef obs_output_slots_scaled_axis_set
#undef obs_output_slots_default_index
#undef obs_output_slots_audio_index_ok
#undef obs_output_slots_video_index_ok
#undef obs_output_slots_audio_type_ok
#undef obs_output_slots_video_type_ok
#undef obs_output_slots_mixer_mask
#undef obs_output_slots_mixer_in_range
#undef obs_output_slots_first_audio
#undef obs_output_slots_first_video

/* In the order of rust/obs-c-oracle/src/output_slots.rs. */
enum {
	ORACLE_OUTPUT_SLOTS_CONSTANT_COUNT = 7,
};

long long oracle_output_slots_constant(size_t which);

long long oracle_output_slots_constant(size_t which)
{
	const long long values[ORACLE_OUTPUT_SLOTS_CONSTANT_COUNT] = {
		OBS_OUTPUT_SLOTS_MULTI_TRACK_AUDIO,  OBS_OUTPUT_SLOTS_MULTI_TRACK_VIDEO,
		OBS_OUTPUT_SLOTS_MAX_VIDEO_ENCODERS, OBS_OUTPUT_SLOTS_MAX_AUDIO_ENCODERS,
		OBS_OUTPUT_SLOTS_MAX_AUDIO_MIXES,    OBS_OUTPUT_SLOTS_ENCODER_AUDIO,
		OBS_OUTPUT_SLOTS_ENCODER_VIDEO,
	};
	return which < ORACLE_OUTPUT_SLOTS_CONSTANT_COUNT ? values[which] : -1;
}
