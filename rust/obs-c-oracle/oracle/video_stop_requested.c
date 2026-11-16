/* The original stop check (the C reference) and the production bridge's mix
 * list accessors, over the real private core layout. */
#define obs_video_stop_requested oracle_obs_video_stop_requested
#include "libobs/obs-video-stop-requested-reference.c.h"
#include "libobs/obs-video-stop-requested.c.h"
#undef obs_video_stop_requested
#include <stdlib.h>

void *oracle_stop_requested_new(size_t count);
void oracle_stop_requested_free(void *core);
void *oracle_stop_requested_mix_video(void *core, size_t index);
bool oracle_stop_requested_locked(void *core);
bool oracle_stop_requested_original(void *core, bool (*stopped)(video_t *));
void oracle_stop_requested_lock(void *core);
void oracle_stop_requested_unlock(void *core);
size_t oracle_stop_requested_count(void *core);
void *oracle_stop_requested_video(void *core, size_t index);

/* A core with count mixes, each with its own video token (never a real
 * output: the query is the caller's). */
void *oracle_stop_requested_new(size_t count)
{
	struct obs_core *core = calloc(1, sizeof(*core));
	if (!core)
		return NULL;
	if (pthread_mutex_init(&core->video.mixes_mutex, NULL) != 0) {
		free(core);
		return NULL;
	}
	if (count) {
		core->video.mixes.array = calloc(count, sizeof(*core->video.mixes.array));
		for (size_t i = 0; core->video.mixes.array && i < count; i++) {
			struct obs_core_video_mix *mix = calloc(1, sizeof(*mix));
			if (!mix)
				break;
			mix->video = (video_t *)(uintptr_t)(0x1000 + 0x10 * i);
			core->video.mixes.array[i] = mix;
			core->video.mixes.num = i + 1;
		}
		core->video.mixes.capacity = count;
	}
	return core;
}

void oracle_stop_requested_free(void *opaque)
{
	struct obs_core *core = opaque;
	for (size_t i = 0; i < core->video.mixes.num; i++)
		free(core->video.mixes.array[i]);
	free(core->video.mixes.array);
	pthread_mutex_destroy(&core->video.mixes_mutex);
	free(core);
}

void *oracle_stop_requested_mix_video(void *opaque, size_t index)
{
	struct obs_core *core = opaque;
	return core->video.mixes.array[index]->video;
}

/* Whether mixes_mutex is held (by anyone, this thread included). */
bool oracle_stop_requested_locked(void *opaque)
{
	struct obs_core *core = opaque;
	if (pthread_mutex_trylock(&core->video.mixes_mutex) != 0)
		return true;
	pthread_mutex_unlock(&core->video.mixes_mutex);
	return false;
}

bool oracle_stop_requested_original(void *core, bool (*stopped)(video_t *))
{
	return oracle_obs_video_stop_requested(core, NULL, NULL, NULL, NULL, stopped);
}

void oracle_stop_requested_lock(void *core)
{
	stop_requested_lock(core);
}

void oracle_stop_requested_unlock(void *core)
{
	stop_requested_unlock(core);
}

size_t oracle_stop_requested_count(void *core)
{
	return stop_requested_count(core);
}

void *oracle_stop_requested_video(void *core, size_t index)
{
	return stop_requested_video(core, index);
}
