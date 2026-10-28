/* Test-only oracle: the original libobs/obs-video-frame-timing.c.h,
 * unmodified, with its two functions renamed to oracle_* so they can link
 * next to the Rust shims, plus the layouts of its structs as the C sees
 * them. */
#include <stddef.h>
#include <stdint.h>

#define obs_video_frame_timing_tick oracle_obs_video_frame_timing_tick
#define obs_video_frame_timing_stats oracle_obs_video_frame_timing_stats

#include "obs-video-frame-timing.c.h"

#undef obs_video_frame_timing_stats
#undef obs_video_frame_timing_tick

/* In the order of rust/obs-c-oracle/src/video_frame_timing.rs. */
enum {
	ORACLE_VIDEO_FRAME_TIMING_LAYOUT_COUNT = 15,
};

size_t oracle_video_frame_timing_layout(size_t which);

size_t oracle_video_frame_timing_layout(size_t which)
{
	const size_t values[ORACLE_VIDEO_FRAME_TIMING_LAYOUT_COUNT] = {
		sizeof(struct obs_video_frame_tick),
		_Alignof(struct obs_video_frame_tick),
		offsetof(struct obs_video_frame_tick, delta_ns),
		offsetof(struct obs_video_frame_tick, seconds),
		sizeof(struct obs_video_frame_sums),
		_Alignof(struct obs_video_frame_sums),
		offsetof(struct obs_video_frame_sums, frame_time_total_ns),
		offsetof(struct obs_video_frame_sums, fps_total_ns),
		offsetof(struct obs_video_frame_sums, fps_total_frames),
		sizeof(struct obs_video_frame_stats),
		_Alignof(struct obs_video_frame_stats),
		offsetof(struct obs_video_frame_stats, sums),
		offsetof(struct obs_video_frame_stats, report),
		offsetof(struct obs_video_frame_stats, fps),
		offsetof(struct obs_video_frame_stats, avg_frame_time_ns),
	};
	return which < ORACLE_VIDEO_FRAME_TIMING_LAYOUT_COUNT ? values[which] : (size_t)-1;
}
