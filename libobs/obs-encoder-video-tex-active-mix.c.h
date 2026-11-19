#include "libobs/build/libobs.pch.h"

struct obs_core_video_mix *get_mix_for_video(video_t *v)
{
	struct obs_core_video_mix *result = NULL;

	pthread_mutex_lock(&obs->video.mixes_mutex);
	for (size_t i = 0, num = obs->video.mixes.num; i < num; i++) {
		struct obs_core_video_mix *mix = obs->video.mixes.array[i];

		if (v == mix->video) {
			result = mix;
			break;
		}
	}
	pthread_mutex_unlock(&obs->video.mixes_mutex);

	return result;
}
