/* Test-only oracle: the original libobs/obs-audio-mix.c.h, unmodified, with
 * its five functions renamed to oracle_* so they can link next to the Rust
 * shims. util_mul_div64, audio_frames_to_ns, ns_to_audio_frames and
 * deque_push_front stay the header inlines; the deque's bmem calls resolve
 * to the allocator of the test binary, which the shims use too. */
#include <stdbool.h>
#include <stddef.h>

#define obs_audio_mix_time_to_frames oracle_obs_audio_mix_time_to_frames
#define obs_audio_mix_add oracle_obs_audio_mix_add
#define obs_audio_buffering_maxed oracle_obs_audio_buffering_maxed
#define obs_audio_buffering_fixed oracle_obs_audio_buffering_fixed
#define obs_audio_buffering_add oracle_obs_audio_buffering_add

#include "obs-audio-mix.c.h"

#undef obs_audio_buffering_add
#undef obs_audio_buffering_fixed
#undef obs_audio_buffering_maxed
#undef obs_audio_mix_add
#undef obs_audio_mix_time_to_frames

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/audio_mix.rs. */
enum {
	ORACLE_AUDIO_MIX_LAYOUT_COUNT = 27,
};

size_t oracle_audio_mix_layout(size_t which);

size_t oracle_audio_mix_layout(size_t which)
{
	const size_t values[ORACLE_AUDIO_MIX_LAYOUT_COUNT] = {
		sizeof(struct ts_info),
		_Alignof(struct ts_info),
		offsetof(struct ts_info, start),
		offsetof(struct ts_info, end),
		sizeof(struct audio_output_data),
		_Alignof(struct audio_output_data),
		offsetof(struct audio_output_data, data),
		sizeof(struct obs_audio_buffering),
		_Alignof(struct obs_audio_buffering),
		offsetof(struct obs_audio_buffering, buffered_ts),
		offsetof(struct obs_audio_buffering, buffered_timestamps),
		offsetof(struct obs_audio_buffering, buffering_wait_ticks),
		offsetof(struct obs_audio_buffering, total_buffering_ticks),
		offsetof(struct obs_audio_buffering, max_buffering_ticks),
		sizeof(struct obs_audio_buffering_step),
		_Alignof(struct obs_audio_buffering_step),
		offsetof(struct obs_audio_buffering_step, added),
		offsetof(struct obs_audio_buffering_step, max_reached),
		offsetof(struct obs_audio_buffering_step, ticks),
		offsetof(struct obs_audio_buffering_step, ms),
		offsetof(struct obs_audio_buffering_step, total_ms),
		sizeof(struct deque),
		_Alignof(struct deque),
		AUDIO_OUTPUT_FRAMES,
		MAX_AUDIO_MIXES,
		MAX_AUDIO_CHANNELS,
		sizeof(int),
	};
	return which < ORACLE_AUDIO_MIX_LAYOUT_COUNT ? values[which] : 0;
}
