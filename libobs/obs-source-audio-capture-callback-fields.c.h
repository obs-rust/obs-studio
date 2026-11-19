#include "libobs/build/libobs.pch.h"

struct darray *obs_source_audio_capture_callback_fields(obs_source_t *source)
{
	return &source->audio_cb_list.da;
}

void obs_source_audio_capture_callback_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->audio_cb_mutex);
}

void obs_source_audio_capture_callback_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->audio_cb_mutex);
}
