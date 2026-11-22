#include "libobs/build/libobs.pch.h"

bool obs_video_active(void)
{
	bool result = false;

	pthread_mutex_lock(&obs->video.mixes_mutex);
	for (size_t i = 0, num = obs->video.mixes.num; i < num; i++) {
		struct obs_core_video_mix *video = obs->video.mixes.array[i];

		if (os_atomic_load_long(&video->raw_active) > 0 ||
		    os_atomic_load_long(&video->gpu_encoder_active) > 0) {
			result = true;
			break;
		}
	}
	pthread_mutex_unlock(&obs->video.mixes_mutex);

	return result;
}
