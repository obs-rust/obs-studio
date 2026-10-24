/* Test-only oracle: the original libobs/obs-source-transition-render.c.h,
 * unmodified, with its nine functions renamed to oracle_* so they can link
 * next to the Rust shims. */
#include <float.h>
#include <stddef.h>

#define obs_source_transition_render_video_due oracle_obs_source_transition_render_video_due
#define obs_source_transition_render_video_stop oracle_obs_source_transition_render_video_stop
#define obs_source_transition_render_draw oracle_obs_source_transition_render_draw
#define obs_source_transition_render_direct_index oracle_obs_source_transition_render_direct_index
#define obs_source_transition_render_audio_stop oracle_obs_source_transition_render_audio_stop
#define obs_source_transition_render_audio_mix oracle_obs_source_transition_render_audio_mix
#define obs_source_transition_render_audio_valid oracle_obs_source_transition_render_audio_valid
#define obs_source_transition_render_audio_place oracle_obs_source_transition_render_audio_place
#define obs_source_transition_render_offset oracle_obs_source_transition_render_offset

#include "obs-source-transition-render.c.h"

#undef obs_source_transition_render_offset
#undef obs_source_transition_render_audio_place
#undef obs_source_transition_render_audio_valid
#undef obs_source_transition_render_audio_mix
#undef obs_source_transition_render_audio_stop
#undef obs_source_transition_render_direct_index
#undef obs_source_transition_render_draw
#undef obs_source_transition_render_video_stop
#undef obs_source_transition_render_video_due

/* The layouts and values the C uses, in the order of
 * rust/obs-c-oracle/src/source_transition_render.rs. The renames above also
 * rename the struct obs_source_transition_render_offset and the
 * enum obs_source_transition_render_audio_place tags, which share their
 * function's name, so the table names the renamed tags. */
enum {
	ORACLE_SOURCE_TRANSITION_RENDER_LAYOUT_COUNT = 27,
};

size_t oracle_source_transition_render_layout(size_t which);

size_t oracle_source_transition_render_layout(size_t which)
{
	const size_t values[ORACLE_SOURCE_TRANSITION_RENDER_LAYOUT_COUNT] = {
		OBS_SOURCE_TRANSITION_RENDER_TARGET_B,
		OBS_SOURCE_TRANSITION_RENDER_AUDIO_FRAMES,
		sizeof(enum obs_source_transition_render_video_step),
		OBS_SOURCE_TRANSITION_RENDER_VIDEO_END,
		OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP,
		OBS_SOURCE_TRANSITION_RENDER_VIDEO_STOP_AUDIO,
		sizeof(enum obs_source_transition_render_draw_step),
		OBS_SOURCE_TRANSITION_RENDER_DRAW_CALLBACK,
		OBS_SOURCE_TRANSITION_RENDER_DRAW_B,
		OBS_SOURCE_TRANSITION_RENDER_DRAW_A,
		sizeof(enum obs_source_transition_render_audio_step),
		OBS_SOURCE_TRANSITION_RENDER_AUDIO_KEEP,
		OBS_SOURCE_TRANSITION_RENDER_AUDIO_END,
		OBS_SOURCE_TRANSITION_RENDER_AUDIO_STOP,
		OBS_SOURCE_TRANSITION_RENDER_MIX_A,
		OBS_SOURCE_TRANSITION_RENDER_MIX_B,
		OBS_SOURCE_TRANSITION_RENDER_COPY_A,
		sizeof(enum oracle_obs_source_transition_render_audio_place),
		OBS_SOURCE_TRANSITION_RENDER_PLACE_NONE,
		OBS_SOURCE_TRANSITION_RENDER_PLACE_LATE,
		OBS_SOURCE_TRANSITION_RENDER_PLACE_MIX,
		sizeof(struct oracle_obs_source_transition_render_offset),
		offsetof(struct oracle_obs_source_transition_render_offset, y),
		sizeof(int),
		sizeof(bool),
		sizeof(size_t),
		FLT_EVAL_METHOD < 0 ? 99 : (size_t)FLT_EVAL_METHOD,
	};
	return which < ORACLE_SOURCE_TRANSITION_RENDER_LAYOUT_COUNT ? values[which] : 0;
}
