#include "libobs/build/libobs.pch.h"

void obs_source_async_decoupled_store(obs_source_t *source, bool value)
{
	source->async_decoupled = value;
}

void obs_source_async_decoupled_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->audio_buf_mutex);
}

void obs_source_async_decoupled_invalidate(obs_source_t *source)
{
	source->timing_set = false;
}

void obs_source_async_decoupled_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->audio_buf_mutex);
}
