#include "libobs/build/libobs.pch.h"

static inline bool stop_requested(void)
{
	bool success = true;

	pthread_mutex_lock(&obs->video.mixes_mutex);
	for (size_t i = 0, num = obs->video.mixes.num; i < num; i++)
		if (!video_output_stopped(obs->video.mixes.array[i]->video))
			success = false;
	pthread_mutex_unlock(&obs->video.mixes_mutex);

	return success;
}
