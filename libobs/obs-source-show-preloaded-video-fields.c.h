#include "libobs/build/libobs.pch.h"

/* Private adapters of the Rust obs_source_show_preloaded_video()
 * (obs-internal.h), each the line of obs-source-show-preloaded-video.c.h it
 * stands for. destroying() is obs_source_caption_callback_destroying()'s.
 * NULL is rejected first. */

bool obs_source_preloaded_video_has_frame(const obs_source_t *source)
{
	return source->async_preload_frame != NULL;
}

void obs_source_preloaded_video_enter_graphics(void)
{
	obs_enter_graphics();
}

void obs_source_preloaded_video_set_texture_size(obs_source_t *source)
{
	set_async_texture_size(source, source->async_preload_frame);
}

void obs_source_preloaded_video_update_textures(obs_source_t *source)
{
	update_async_textures(source, source->async_preload_frame, source->async_textures, source->async_texrender);
}

void obs_source_preloaded_video_mark_fresh(obs_source_t *source)
{
	os_atomic_set_bool(&source->async_textures_stale, false);
}

void obs_source_preloaded_video_mark_active(obs_source_t *source)
{
	os_atomic_set_bool(&source->async_active, true);
}

void obs_source_preloaded_video_leave_graphics(void)
{
	obs_leave_graphics();
}

void obs_source_preloaded_video_lock_async(obs_source_t *source)
{
	pthread_mutex_lock(&source->async_mutex);
}

uint64_t obs_source_preloaded_video_last_frame_ts(const obs_source_t *source)
{
	return source->last_frame_ts;
}

void obs_source_preloaded_video_unlock_async(obs_source_t *source)
{
	pthread_mutex_unlock(&source->async_mutex);
}

void obs_source_preloaded_video_lock_audio(obs_source_t *source)
{
	pthread_mutex_lock(&source->audio_buf_mutex);
}

bool obs_source_preloaded_video_monitor_only(const obs_source_t *source)
{
	return source->monitoring_type == OBS_MONITORING_TYPE_MONITOR_ONLY;
}

uint64_t obs_source_preloaded_video_now(void)
{
	return os_gettime_ns();
}

void obs_source_preloaded_video_reset_timing(obs_source_t *source, uint64_t timestamp, uint64_t os_time)
{
	reset_audio_timing(source, timestamp, os_time);
}

void obs_source_preloaded_video_reset_data(obs_source_t *source, uint64_t os_time)
{
	reset_audio_data_at_sys_time(source, os_time);
}

void obs_source_preloaded_video_unlock_audio(obs_source_t *source)
{
	pthread_mutex_unlock(&source->audio_buf_mutex);
}
