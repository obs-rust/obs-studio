/* Test-only oracle: the original libobs/obs-video-pacing.c.h, unmodified,
 * with its two functions renamed to oracle_* so they can link next to the
 * Rust shims, plus the layout of its struct as the C sees it. The struct tag
 * shares its function's name, so the rename renames it too: the layout table
 * names the renamed tag. */
#include <stddef.h>
#include <stdint.h>

#define obs_video_pacing_step oracle_obs_video_pacing_step
#define obs_video_pacing_group_starts oracle_obs_video_pacing_group_starts

#include "obs-video-pacing.c.h"

#undef obs_video_pacing_group_starts
#undef obs_video_pacing_step

/* In the order of rust/obs-c-oracle/src/video_pacing.rs. */
enum {
	ORACLE_VIDEO_PACING_LAYOUT_COUNT = 4,
};

size_t oracle_video_pacing_layout(size_t which);

size_t oracle_video_pacing_layout(size_t which)
{
	const size_t values[ORACLE_VIDEO_PACING_LAYOUT_COUNT] = {
		sizeof(struct oracle_obs_video_pacing_step),
		_Alignof(struct oracle_obs_video_pacing_step),
		offsetof(struct oracle_obs_video_pacing_step, frame_time),
		offsetof(struct oracle_obs_video_pacing_step, count),
	};
	return which < ORACLE_VIDEO_PACING_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
