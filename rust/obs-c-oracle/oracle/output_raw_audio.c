/* Test-only oracle: the original libobs/obs-output-raw-audio.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims, plus its constants as the C sees them. Nothing in it
 * divides, so no input traps on any host. obs_output_raw_audio_mix_on()
 * shifts by idx: callers keep idx below the width of size_t, where the C is
 * defined. */
#include <stddef.h>

#define obs_output_raw_audio_passthrough oracle_obs_output_raw_audio_passthrough
#define obs_output_raw_audio_video_unset oracle_obs_output_raw_audio_video_unset
#define obs_output_raw_audio_before_video oracle_obs_output_raw_audio_before_video
#define obs_output_raw_audio_ends_before oracle_obs_output_raw_audio_ends_before
#define obs_output_raw_audio_cut_ns oracle_obs_output_raw_audio_cut_ns
#define obs_output_raw_audio_cut_ts oracle_obs_output_raw_audio_cut_ts
#define obs_output_raw_audio_cut_bytes oracle_obs_output_raw_audio_cut_bytes
#define obs_output_raw_audio_cut_frames oracle_obs_output_raw_audio_cut_frames
#define obs_output_raw_audio_start_unset oracle_obs_output_raw_audio_start_unset
#define obs_output_raw_audio_block_bytes oracle_obs_output_raw_audio_block_bytes
#define obs_output_raw_audio_push_bytes oracle_obs_output_raw_audio_push_bytes
#define obs_output_raw_audio_can_pop oracle_obs_output_raw_audio_can_pop
#define obs_output_raw_audio_block_ts oracle_obs_output_raw_audio_block_ts
#define obs_output_raw_audio_paused_ts oracle_obs_output_raw_audio_paused_ts
#define obs_output_raw_audio_next_frames oracle_obs_output_raw_audio_next_frames
#define obs_output_raw_audio_mix_on oracle_obs_output_raw_audio_mix_on
#define obs_output_raw_audio_interleave oracle_obs_output_raw_audio_interleave
#define obs_output_raw_audio_delay_on oracle_obs_output_raw_audio_delay_on
#define obs_output_raw_audio_delay_ns oracle_obs_output_raw_audio_delay_ns

#include "obs-output-raw-audio.c.h"

#undef obs_output_raw_audio_delay_ns
#undef obs_output_raw_audio_delay_on
#undef obs_output_raw_audio_interleave
#undef obs_output_raw_audio_mix_on
#undef obs_output_raw_audio_next_frames
#undef obs_output_raw_audio_paused_ts
#undef obs_output_raw_audio_block_ts
#undef obs_output_raw_audio_can_pop
#undef obs_output_raw_audio_push_bytes
#undef obs_output_raw_audio_block_bytes
#undef obs_output_raw_audio_start_unset
#undef obs_output_raw_audio_cut_frames
#undef obs_output_raw_audio_cut_bytes
#undef obs_output_raw_audio_cut_ts
#undef obs_output_raw_audio_cut_ns
#undef obs_output_raw_audio_ends_before
#undef obs_output_raw_audio_before_video
#undef obs_output_raw_audio_video_unset
#undef obs_output_raw_audio_passthrough

/* In the order of rust/obs-c-oracle/src/output_raw_audio.rs. */
enum {
	ORACLE_OUTPUT_RAW_AUDIO_CONSTANT_COUNT = 2,
};

unsigned long long oracle_output_raw_audio_constant(size_t which);

unsigned long long oracle_output_raw_audio_constant(size_t which)
{
	const unsigned long long values[ORACLE_OUTPUT_RAW_AUDIO_CONSTANT_COUNT] = {
		OBS_OUTPUT_RAW_AUDIO_FLAG_VIDEO,
		OBS_OUTPUT_RAW_AUDIO_FRAMES,
	};
	return which < ORACLE_OUTPUT_RAW_AUDIO_CONSTANT_COUNT ? values[which] : (unsigned long long)-1;
}
