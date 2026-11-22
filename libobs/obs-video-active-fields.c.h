#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_video_active(): the core's mixes_mutex,
 * the mix list's length and entries, and each mix's raw_active and
 * gpu_encoder_active counts (atomic long loads, as the original did). The
 * list and the mix layout stay in C; Rust decides the order of the reads. */
void obs_video_active_lock(void);
void obs_video_active_lock(void)
{
	pthread_mutex_lock(&obs->video.mixes_mutex);
}

void obs_video_active_unlock(void);
void obs_video_active_unlock(void)
{
	pthread_mutex_unlock(&obs->video.mixes_mutex);
}

size_t obs_video_active_count(void);
size_t obs_video_active_count(void)
{
	return obs->video.mixes.num;
}

struct obs_core_video_mix *obs_video_active_at(size_t index);
struct obs_core_video_mix *obs_video_active_at(size_t index)
{
	return obs->video.mixes.array[index];
}

long obs_video_active_raw(struct obs_core_video_mix *mix);
long obs_video_active_raw(struct obs_core_video_mix *mix)
{
	return os_atomic_load_long(&mix->raw_active);
}

long obs_video_active_gpu(struct obs_core_video_mix *mix);
long obs_video_active_gpu(struct obs_core_video_mix *mix)
{
	return os_atomic_load_long(&mix->gpu_encoder_active);
}
