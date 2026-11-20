#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust video-statistics queries (#5364): the current
 * core's stats, and its five obs_video_stats_* readers, which are static
 * inline in obs-video-stats.h. Each reader keeps its one lock/read/unlock
 * (#175); the stats layout and mutex stay in C. A live core is required, as
 * in the original: the address is formed without a NULL check. */
struct obs_video_stats *obs_video_stats_queries_stats(void);
struct obs_video_stats *obs_video_stats_queries_stats(void)
{
	return &obs->video.stats;
}

uint64_t obs_video_stats_queries_frame_time(struct obs_video_stats *stats);
uint64_t obs_video_stats_queries_frame_time(struct obs_video_stats *stats)
{
	return obs_video_stats_frame_time(stats);
}

double obs_video_stats_queries_fps(struct obs_video_stats *stats);
double obs_video_stats_queries_fps(struct obs_video_stats *stats)
{
	return obs_video_stats_fps(stats);
}

uint64_t obs_video_stats_queries_avg_frame_time_ns(struct obs_video_stats *stats);
uint64_t obs_video_stats_queries_avg_frame_time_ns(struct obs_video_stats *stats)
{
	return obs_video_stats_avg_frame_time_ns(stats);
}

uint32_t obs_video_stats_queries_total_frames(struct obs_video_stats *stats);
uint32_t obs_video_stats_queries_total_frames(struct obs_video_stats *stats)
{
	return obs_video_stats_total_frames(stats);
}

uint32_t obs_video_stats_queries_lagged_frames(struct obs_video_stats *stats);
uint32_t obs_video_stats_queries_lagged_frames(struct obs_video_stats *stats)
{
	return obs_video_stats_lagged_frames(stats);
}
