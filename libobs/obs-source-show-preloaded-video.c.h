#include "libobs/build/libobs.pch.h"

void obs_source_show_preloaded_video(obs_source_t *source)
{
	uint64_t last_frame_ts;
	uint64_t sys_ts;

	if (!obs_source_valid(source, "obs_source_show_preloaded_video"))
		return;
	if (destroying(source))
		return;
	if (!source->async_preload_frame)
		return;

	obs_enter_graphics();

	set_async_texture_size(source, source->async_preload_frame);
	update_async_textures(source, source->async_preload_frame, source->async_textures, source->async_texrender);
	os_atomic_set_bool(&source->async_textures_stale, false);
	os_atomic_set_bool(&source->async_active, true);

	obs_leave_graphics();

	pthread_mutex_lock(&source->async_mutex);
	last_frame_ts = source->last_frame_ts;
	pthread_mutex_unlock(&source->async_mutex);

	pthread_mutex_lock(&source->audio_buf_mutex);
	sys_ts = (source->monitoring_type != OBS_MONITORING_TYPE_MONITOR_ONLY) ? os_gettime_ns() : 0;
	reset_audio_timing(source, last_frame_ts, sys_ts);
	reset_audio_data_at_sys_time(source, sys_ts);
	pthread_mutex_unlock(&source->audio_buf_mutex);
}
