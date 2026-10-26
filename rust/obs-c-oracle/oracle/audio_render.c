/* Test-only oracle: the original libobs/obs-audio-render.c.h, unmodified,
 * with its four functions renamed to oracle_* so they can link next to the
 * Rust shims. close_float stays the header inline. */
#include <stdbool.h>
#include <stddef.h>

#define obs_audio_render_tree oracle_obs_audio_render_tree
#define obs_audio_render_silence oracle_obs_audio_render_silence
#define obs_audio_render_clear_mixes oracle_obs_audio_render_clear_mixes
#define obs_audio_render_backwards oracle_obs_audio_render_backwards

#include "obs-audio-render.c.h"

#undef obs_audio_render_backwards
#undef obs_audio_render_clear_mixes
#undef obs_audio_render_silence
#undef obs_audio_render_tree

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/audio_render.rs. */
enum {
	ORACLE_AUDIO_RENDER_LAYOUT_COUNT = 18,
};

size_t oracle_audio_render_layout(size_t which);

size_t oracle_audio_render_layout(size_t which)
{
	const size_t values[ORACLE_AUDIO_RENDER_LAYOUT_COUNT] = {
		sizeof(enum obs_audio_tree_choice),
		OBS_AUDIO_TREE_NONE,
		OBS_AUDIO_TREE_SKIP,
		OBS_AUDIO_TREE_ADD,
		OBS_AUDIO_TREE_DUPLICATE,
		sizeof(enum obs_audio_backwards_choice),
		OBS_AUDIO_BACKWARDS_NONE,
		OBS_AUDIO_BACKWARDS_PENDING,
		OBS_AUDIO_BACKWARDS_IGNORE,
		sizeof(struct obs_audio_monitor_dedup),
		offsetof(struct obs_audio_monitor_dedup, set),
		offsetof(struct obs_audio_monitor_dedup, active),
		offsetof(struct obs_audio_monitor_dedup, muted),
		offsetof(struct obs_audio_monitor_dedup, volume),
		offsetof(struct obs_audio_monitor_dedup, mixers),
		OBS_AUDIO_RENDER_TYPE_INPUT,
		OBS_AUDIO_RENDER_SOURCE_AUDIO,
		OBS_AUDIO_RENDER_SOURCE_COMPOSITE,
	};
	return which < ORACLE_AUDIO_RENDER_LAYOUT_COUNT ? values[which] : 0;
}
