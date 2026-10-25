/* Test-only oracle: the original libobs/obs-audio-discard.c.h, unmodified,
 * with its three functions renamed to oracle_* so they can link next to the
 * Rust shims. util_mul_div64 and deque_pop_front stay the header inlines;
 * the deque's bmem calls resolve to the allocator of the test binary,
 * which the shims use too. */
#include <stdbool.h>
#include <stddef.h>

#define obs_audio_discard_ignore oracle_obs_audio_discard_ignore
#define obs_audio_discard_if_stopped oracle_obs_audio_discard_if_stopped
#define obs_audio_discard_tick oracle_obs_audio_discard_tick

#include "obs-audio-discard.c.h"

#undef obs_audio_discard_tick
#undef obs_audio_discard_if_stopped
#undef obs_audio_discard_ignore

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/audio_discard.rs. */
enum {
	ORACLE_AUDIO_DISCARD_LAYOUT_COUNT = 31,
};

size_t oracle_audio_discard_layout(size_t which);

size_t oracle_audio_discard_layout(size_t which)
{
	const size_t values[ORACLE_AUDIO_DISCARD_LAYOUT_COUNT] = {
		sizeof(struct obs_audio_input),
		_Alignof(struct obs_audio_input),
		offsetof(struct obs_audio_input, buf),
		offsetof(struct obs_audio_input, audio_ts),
		offsetof(struct obs_audio_input, last_size),
		offsetof(struct obs_audio_input, pending),
		offsetof(struct obs_audio_input, pending_stop),
		offsetof(struct obs_audio_input, timing_set),
		sizeof(struct obs_audio_ignore_step),
		_Alignof(struct obs_audio_ignore_step),
		offsetof(struct obs_audio_ignore_step, rerender),
		offsetof(struct obs_audio_ignore_step, no_timestamp),
		offsetof(struct obs_audio_ignore_step, buffered),
		offsetof(struct obs_audio_ignore_step, dropped),
		offsetof(struct obs_audio_ignore_step, lag),
		offsetof(struct obs_audio_ignore_step, warn),
		offsetof(struct obs_audio_ignore_step, warn_lag),
		sizeof(struct obs_audio_discard_step),
		_Alignof(struct obs_audio_discard_step),
		offsetof(struct obs_audio_discard_step, path),
		offsetof(struct obs_audio_discard_step, stop),
		sizeof(enum obs_audio_stop_check),
		sizeof(enum obs_audio_discard_path),
		OBS_AUDIO_STOP_NONE,
		OBS_AUDIO_STOP_PENDING,
		OBS_AUDIO_STOP_CLEARED,
		OBS_AUDIO_DISCARD_RENDER_SOURCE,
		OBS_AUDIO_DISCARD_AHEAD,
		OBS_AUDIO_DISCARD_BEHIND,
		OBS_AUDIO_DISCARD_START_AT_END,
		OBS_AUDIO_DISCARD_PENDING,
	};
	return which < ORACLE_AUDIO_DISCARD_LAYOUT_COUNT ? values[which] : 0;
}
