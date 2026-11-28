#include "libobs/build/libobs.pch.h"

/* Private adapters of the Rust obs_source_output_video_internal()
 * (obs-internal.h, no EXPORT; obs-source::ffi::source_output_video_internal,
 * #5841), each the line of obs-source-output-video-internal.c.h it stands
 * for. The async frame cache (cache_video(), free_async_cache()) stays in
 * obs-source.c.h. The caller rejects a NULL source first; the frame
 * adapters take non-NULL frames only. */
void obs_source_output_internal_lock_audio_buf(obs_source_t *source);
void obs_source_output_internal_lock_audio_buf(obs_source_t *source)
{
	pthread_mutex_lock(&source->audio_buf_mutex);
}

uint64_t obs_source_output_internal_frame_timestamp(const struct obs_source_frame *frame);
uint64_t obs_source_output_internal_frame_timestamp(const struct obs_source_frame *frame)
{
	return frame->timestamp;
}

void obs_source_output_internal_set_last_output_frame_ts(obs_source_t *source, uint64_t timestamp);
void obs_source_output_internal_set_last_output_frame_ts(obs_source_t *source, uint64_t timestamp)
{
	source->last_output_frame_ts = timestamp;
}

void obs_source_output_internal_unlock_audio_buf(obs_source_t *source);
void obs_source_output_internal_unlock_audio_buf(obs_source_t *source)
{
	pthread_mutex_unlock(&source->audio_buf_mutex);
}

void obs_source_output_internal_lock_async(obs_source_t *source);
void obs_source_output_internal_lock_async(obs_source_t *source)
{
	pthread_mutex_lock(&source->async_mutex);
}

void obs_source_output_internal_set_async_active(obs_source_t *source, bool active);
void obs_source_output_internal_set_async_active(obs_source_t *source, bool active)
{
	os_atomic_set_bool(&source->async_active, active);
}

void obs_source_output_internal_mark_stale(obs_source_t *source);
void obs_source_output_internal_mark_stale(obs_source_t *source)
{
	os_atomic_set_bool(&source->async_textures_stale, true);
}

void obs_source_output_internal_clear_last_frame_ts(obs_source_t *source);
void obs_source_output_internal_clear_last_frame_ts(obs_source_t *source)
{
	source->last_frame_ts = 0;
}

void obs_source_output_internal_free_cache(obs_source_t *source);
void obs_source_output_internal_free_cache(obs_source_t *source)
{
	free_async_cache(source);
}

void obs_source_output_internal_unlock_async(obs_source_t *source);
void obs_source_output_internal_unlock_async(obs_source_t *source)
{
	pthread_mutex_unlock(&source->async_mutex);
}

void obs_source_output_internal_frame_received(obs_source_t *source);
void obs_source_output_internal_frame_received(obs_source_t *source)
{
	source_profiler_async_frame_received(source);
}

struct obs_source_frame *obs_source_output_internal_cache_video(obs_source_t *source,
								const struct obs_source_frame *frame);
struct obs_source_frame *obs_source_output_internal_cache_video(obs_source_t *source,
								const struct obs_source_frame *frame)
{
	return cache_video(source, frame);
}

long obs_source_output_internal_release_ref(struct obs_source_frame *frame);
long obs_source_output_internal_release_ref(struct obs_source_frame *frame)
{
	return os_atomic_dec_long(&frame->refs);
}

void obs_source_output_internal_destroy_frame(struct obs_source_frame *frame);
void obs_source_output_internal_destroy_frame(struct obs_source_frame *frame)
{
	obs_source_frame_destroy(frame);
}

void obs_source_output_internal_push_frame(obs_source_t *source, struct obs_source_frame *frame);
void obs_source_output_internal_push_frame(obs_source_t *source, struct obs_source_frame *frame)
{
	da_push_back(source->async_frames, &frame);
}
