#include "libobs/build/libobs.pch.h"

/* The audio buffer fields obs_source_get_audio_timestamp() and
 * obs_source_get_audio_mix() read (the obs-source shims). */
pthread_mutex_t *obs_source_audio_buf_mutex_field(const obs_source_t *source);
pthread_mutex_t *obs_source_audio_buf_mutex_field(const obs_source_t *source)
{
	/* Locking synchronizes the read without changing the logical source. */
	return (pthread_mutex_t *)(uintptr_t)&source->audio_buf_mutex;
}

const uint64_t *obs_source_audio_ts_field(const obs_source_t *source);
const uint64_t *obs_source_audio_ts_field(const obs_source_t *source)
{
	return &source->audio_ts;
}

/* audio_output_buf[0][0]: MAX_AUDIO_MIXES rows of MAX_AUDIO_CHANNELS. */
float *const *obs_source_audio_output_buf_field(const obs_source_t *source);
float *const *obs_source_audio_output_buf_field(const obs_source_t *source)
{
	return &source->audio_output_buf[0][0];
}
