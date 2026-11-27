#include "libobs/build/libobs.pch.h"

/* obs_source_set_video_frame_internal() (obs-internal.h, no EXPORT), the
 * graphics-thread worker every obs_source_set_video_frame*() entry ends in,
 * with its static helpers frame_copy_shape() and preload_frame_changed(),
 * moved verbatim out of obs-source.c.h (#5839), where nothing else used
 * them. */

static inline struct obs_source_frame_copy_shape frame_copy_shape(const struct obs_source_frame *frame)
{
	const struct obs_source_frame_copy_shape shape = {
		.format = (int)frame->format,
		.width = frame->width,
		.height = frame->height,
	};
	return shape;
}

static inline bool preload_frame_changed(obs_source_t *source, const struct obs_source_frame *in)
{
	const struct obs_source_frame_copy_shape none = {0};
	const bool has_preload = source->async_preload_frame != NULL;

	return obs_source_frame_copy_preload_changed(
		has_preload, has_preload ? frame_copy_shape(source->async_preload_frame) : none, frame_copy_shape(in));
}

void obs_source_set_video_frame_internal(obs_source_t *source, const struct obs_source_frame *frame)
{
	if (!obs_source_valid(source, "obs_source_set_video_frame"))
		return;
	if (!frame)
		return;

	obs_enter_graphics();

	if (preload_frame_changed(source, frame)) {
		obs_source_frame_destroy(source->async_preload_frame);
		source->async_preload_frame = obs_source_frame_create(frame->format, frame->width, frame->height);
	}

	obs_source_frame_copy(source->async_preload_frame, frame);
	set_async_texture_size(source, source->async_preload_frame);
	update_async_textures(source, source->async_preload_frame, source->async_textures, source->async_texrender);
	os_atomic_set_bool(&source->async_textures_stale, false);

	pthread_mutex_lock(&source->async_mutex);
	source->last_frame_ts = frame->timestamp;
	pthread_mutex_unlock(&source->async_mutex);

	obs_leave_graphics();
}
