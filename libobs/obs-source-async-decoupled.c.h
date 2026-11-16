#include "libobs/build/libobs.pch.h"

void obs_source_set_async_decoupled(obs_source_t *source, bool decouple)
{
	if (!obs_ptr_valid(source, "obs_source_set_async_decoupled"))
		return;

	source->async_decoupled = decouple;
	if (decouple) {
		pthread_mutex_lock(&source->audio_buf_mutex);
		source->timing_set = false;
		reset_audio_data(source, 0);
		pthread_mutex_unlock(&source->audio_buf_mutex);
	}
}
