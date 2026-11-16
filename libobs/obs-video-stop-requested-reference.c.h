#include "libobs/build/libobs.pch.h"

/* The original stop check, for the C reference and the Tier 3 oracle. Its body
 * is the C's, verbatim, over the core and the query it is given. */
#define obs stop_requested_core
#define video_output_stopped stop_requested_stopped
static inline bool stop_requested_reference(struct obs_core *obs, bool (*video_output_stopped)(video_t *))
{
	bool success = true;

	pthread_mutex_lock(&obs->video.mixes_mutex);
	for (size_t i = 0, num = obs->video.mixes.num; i < num; i++)
		if (!video_output_stopped(obs->video.mixes.array[i]->video))
			success = false;
	pthread_mutex_unlock(&obs->video.mixes_mutex);

	return success;
}
#undef video_output_stopped
#undef obs

bool obs_video_stop_requested(struct obs_core *core, void (*lock)(struct obs_core *core),
			      void (*unlock)(struct obs_core *core), size_t (*count)(struct obs_core *core),
			      struct video_output *(*video)(struct obs_core *core, size_t index),
			      bool (*stopped)(struct video_output *video))
{
	UNUSED_PARAMETER(lock);
	UNUSED_PARAMETER(unlock);
	UNUSED_PARAMETER(count);
	UNUSED_PARAMETER(video);
	return stop_requested_reference(core, stopped);
}
