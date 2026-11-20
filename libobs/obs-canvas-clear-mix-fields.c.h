#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_canvas_clear_mix(). The canvas's mix
 * slot, the core's mix list, its mutex and the mix teardown
 * (obs_free_video_mix()) stay in C; Rust decides when each is used. As in
 * the original, the slot and the list are read without any other lock, and
 * a NULL canvas is outside the valid domain. */
struct obs_core_video_mix **obs_canvas_clear_mix_slot(obs_canvas_t *canvas);
struct obs_core_video_mix **obs_canvas_clear_mix_slot(obs_canvas_t *canvas)
{
	return &canvas->mix;
}

void obs_canvas_clear_mix_lock(void);
void obs_canvas_clear_mix_lock(void)
{
	pthread_mutex_lock(&obs->video.mixes_mutex);
}

void obs_canvas_clear_mix_unlock(void);
void obs_canvas_clear_mix_unlock(void)
{
	pthread_mutex_unlock(&obs->video.mixes_mutex);
}

size_t obs_canvas_clear_mix_count(void);
size_t obs_canvas_clear_mix_count(void)
{
	return obs->video.mixes.num;
}

struct obs_core_video_mix *obs_canvas_clear_mix_at(size_t index);
struct obs_core_video_mix *obs_canvas_clear_mix_at(size_t index)
{
	return obs->video.mixes.array[index];
}

void obs_canvas_clear_mix_erase(size_t index);
void obs_canvas_clear_mix_erase(size_t index)
{
	da_erase(obs->video.mixes, index);
}

void obs_canvas_clear_mix_free(struct obs_core_video_mix *mix);
void obs_canvas_clear_mix_free(struct obs_core_video_mix *mix)
{
	obs_free_video_mix(mix);
}
