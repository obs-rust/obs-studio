#include "libobs/build/libobs.pch.h"

uint64_t obs_source_get_audio_timestamp(const obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_get_audio_timestamp"))
		return 0;
	/* Locking synchronizes the read without changing the logical source. */
	pthread_mutex_t *mutex = (pthread_mutex_t *)(uintptr_t)&source->audio_buf_mutex;
	pthread_mutex_lock(mutex);
	uint64_t timestamp = source->audio_ts;
	pthread_mutex_unlock(mutex);
	return timestamp;
}

void obs_source_get_audio_mix(const obs_source_t *source, struct obs_source_audio_mix *audio)
{
	if (!obs_source_valid(source, "obs_source_get_audio_mix"))
		return;
	if (!obs_ptr_valid(audio, "audio"))
		return;

	for (size_t mix = 0; mix < MAX_AUDIO_MIXES; mix++) {
		for (size_t ch = 0; ch < MAX_AUDIO_CHANNELS; ch++) {
			audio->output[mix].data[ch] = source->audio_output_buf[mix][ch];
		}
	}
}
