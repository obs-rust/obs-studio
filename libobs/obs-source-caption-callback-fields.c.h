#include "libobs/build/libobs.pch.h"

struct darray *obs_source_caption_callback_fields(obs_source_t *source)
{
	return &source->caption_cb_list.da;
}

void obs_source_caption_callback_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->caption_cb_mutex);
}

void obs_source_caption_callback_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->caption_cb_mutex);
}
