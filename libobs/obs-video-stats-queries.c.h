#include "libobs/build/libobs.pch.h"

uint64_t obs_get_video_frame_time(void)
{
	return obs_video_stats_frame_time(&obs->video.stats);
}

double obs_get_active_fps(void)
{
	return obs_video_stats_fps(&obs->video.stats);
}

uint64_t obs_get_average_frame_time_ns(void)
{
	return obs_video_stats_avg_frame_time_ns(&obs->video.stats);
}

uint32_t obs_get_total_frames(void)
{
	return obs_video_stats_total_frames(&obs->video.stats);
}

uint32_t obs_get_lagged_frames(void)
{
	return obs_video_stats_lagged_frames(&obs->video.stats);
}
