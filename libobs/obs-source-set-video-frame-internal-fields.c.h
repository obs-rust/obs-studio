#include "libobs/build/libobs.pch.h"

/* Private adapters of the Rust obs_source_set_video_frame_internal()
 * (obs-internal.h, no EXPORT; obs-source::ffi::source_set_video_frame_internal,
 * #5839), each the line of obs-source-set-video-frame-internal.c.h it stands
 * for. The preload frame's shapes, destroy, create and copy, async_mutex and
 * the last_frame_ts write are obs-source-preload-video-internal-fields.c.h's
 * (#5837). The caller rejects a NULL source and a NULL frame first. */
void obs_source_set_frame_internal_enter_graphics(void);
void obs_source_set_frame_internal_enter_graphics(void)
{
	obs_enter_graphics();
}

void obs_source_set_frame_internal_set_texture_size(obs_source_t *source);
void obs_source_set_frame_internal_set_texture_size(obs_source_t *source)
{
	set_async_texture_size(source, source->async_preload_frame);
}

void obs_source_set_frame_internal_update_textures(obs_source_t *source);
void obs_source_set_frame_internal_update_textures(obs_source_t *source)
{
	update_async_textures(source, source->async_preload_frame, source->async_textures, source->async_texrender);
}

void obs_source_set_frame_internal_mark_fresh(obs_source_t *source);
void obs_source_set_frame_internal_mark_fresh(obs_source_t *source)
{
	os_atomic_set_bool(&source->async_textures_stale, false);
}

void obs_source_set_frame_internal_leave_graphics(void);
void obs_source_set_frame_internal_leave_graphics(void)
{
	obs_leave_graphics();
}
