#pragma once

#include <pthread.h>
#include <stdint.h>

/* Private graphics-thread clock and statistics read by obs_get_* accessors. */
struct obs_video_stats {
	pthread_mutex_t mutex;
	uint64_t frame_time_ns;
	uint64_t avg_frame_time_ns;
	double fps;
	uint32_t total_frames;
	uint32_t lagged_frames;
};

static inline int obs_video_stats_init(struct obs_video_stats *stats)
{
	return pthread_mutex_init(&stats->mutex, NULL);
}

static inline void obs_video_stats_destroy(struct obs_video_stats *stats)
{
	pthread_mutex_destroy(&stats->mutex);
}

static inline uint64_t obs_video_stats_frame_time(struct obs_video_stats *stats)
{
	pthread_mutex_lock(&stats->mutex);
	uint64_t frame_time = stats->frame_time_ns;
	pthread_mutex_unlock(&stats->mutex);
	return frame_time;
}

static inline void obs_video_stats_set_frame_time(struct obs_video_stats *stats, uint64_t frame_time)
{
	pthread_mutex_lock(&stats->mutex);
	stats->frame_time_ns = frame_time;
	pthread_mutex_unlock(&stats->mutex);
}

static inline void obs_video_stats_add_frames(struct obs_video_stats *stats, uint32_t total, uint32_t lagged)
{
	pthread_mutex_lock(&stats->mutex);
	stats->total_frames += total;
	stats->lagged_frames += lagged;
	pthread_mutex_unlock(&stats->mutex);
}

static inline void obs_video_stats_set_timing(struct obs_video_stats *stats, double fps, uint64_t frame_time_ns)
{
	pthread_mutex_lock(&stats->mutex);
	stats->fps = fps;
	stats->avg_frame_time_ns = frame_time_ns;
	pthread_mutex_unlock(&stats->mutex);
}

static inline uint32_t obs_video_stats_total_frames(struct obs_video_stats *stats)
{
	pthread_mutex_lock(&stats->mutex);
	uint32_t total = stats->total_frames;
	pthread_mutex_unlock(&stats->mutex);
	return total;
}

static inline void obs_video_stats_frame_counts(struct obs_video_stats *stats, uint32_t *total, uint32_t *lagged)
{
	pthread_mutex_lock(&stats->mutex);
	*total = stats->total_frames;
	*lagged = stats->lagged_frames;
	pthread_mutex_unlock(&stats->mutex);
}

static inline uint32_t obs_video_stats_lagged_frames(struct obs_video_stats *stats)
{
	pthread_mutex_lock(&stats->mutex);
	uint32_t lagged = stats->lagged_frames;
	pthread_mutex_unlock(&stats->mutex);
	return lagged;
}

static inline double obs_video_stats_fps(struct obs_video_stats *stats)
{
	pthread_mutex_lock(&stats->mutex);
	double fps = stats->fps;
	pthread_mutex_unlock(&stats->mutex);
	return fps;
}

static inline uint64_t obs_video_stats_avg_frame_time_ns(struct obs_video_stats *stats)
{
	pthread_mutex_lock(&stats->mutex);
	uint64_t frame_time = stats->avg_frame_time_ns;
	pthread_mutex_unlock(&stats->mutex);
	return frame_time;
}
