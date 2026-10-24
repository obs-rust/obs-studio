/* Test-only oracle: the original libobs/obs-scene-audio-window.c.h,
 * unmodified, with its seven functions renamed to oracle_* so they can link
 * next to the Rust shims. ns_to_audio_frames and util_mul_div64 stay the
 * header inlines. */
#include <stddef.h>

#define obs_scene_audio_window_pick oracle_obs_scene_audio_window_pick
#define obs_scene_audio_window_counts oracle_obs_scene_audio_window_counts
#define obs_scene_audio_window_earliest oracle_obs_scene_audio_window_earliest
#define obs_scene_audio_window_silent oracle_obs_scene_audio_window_silent
#define obs_scene_audio_window_frames oracle_obs_scene_audio_window_frames
#define obs_scene_audio_window_mixes oracle_obs_scene_audio_window_mixes
#define obs_scene_audio_window_adds oracle_obs_scene_audio_window_adds

#include "obs-scene-audio-window.c.h"

#undef obs_scene_audio_window_adds
#undef obs_scene_audio_window_mixes
#undef obs_scene_audio_window_frames
#undef obs_scene_audio_window_silent
#undef obs_scene_audio_window_earliest
#undef obs_scene_audio_window_counts
#undef obs_scene_audio_window_pick

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/scene_audio_window.rs. */
enum {
	ORACLE_SCENE_AUDIO_WINDOW_LAYOUT_COUNT = 10,
};

size_t oracle_scene_audio_window_layout(size_t which);

size_t oracle_scene_audio_window_layout(size_t which)
{
	const size_t values[ORACLE_SCENE_AUDIO_WINDOW_LAYOUT_COUNT] = {
		sizeof(enum obs_scene_audio_window_child),
		OBS_SCENE_AUDIO_WINDOW_SOURCE,
		OBS_SCENE_AUDIO_WINDOW_SHOW,
		OBS_SCENE_AUDIO_WINDOW_HIDE,
		sizeof(struct obs_scene_audio_window),
		_Alignof(struct obs_scene_audio_window),
		offsetof(struct obs_scene_audio_window, mix),
		offsetof(struct obs_scene_audio_window, pos),
		offsetof(struct obs_scene_audio_window, count),
		AUDIO_OUTPUT_FRAMES,
	};
	return which < ORACLE_SCENE_AUDIO_WINDOW_LAYOUT_COUNT ? values[which] : 0;
}
