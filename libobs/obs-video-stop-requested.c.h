#include "libobs/build/libobs.pch.h"

/* Private bridge to obs_video_stop_requested() (obs-video-stop-requested.h):
 * these accessors give it the mix list, which stays C-owned. */
static void stop_requested_lock(struct obs_core *core)
{
	pthread_mutex_lock(&core->video.mixes_mutex);
}

static void stop_requested_unlock(struct obs_core *core)
{
	pthread_mutex_unlock(&core->video.mixes_mutex);
}

static size_t stop_requested_count(struct obs_core *core)
{
	return core->video.mixes.num;
}

static video_t *stop_requested_video(struct obs_core *core, size_t index)
{
	return core->video.mixes.array[index]->video;
}

static inline bool stop_requested(void)
{
	return obs_video_stop_requested(obs, stop_requested_lock, stop_requested_unlock, stop_requested_count,
					stop_requested_video, video_output_stopped);
}
