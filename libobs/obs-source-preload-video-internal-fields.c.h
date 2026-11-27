#include "libobs/build/libobs.pch.h"

/* Private adapters of the Rust obs_source_preload_video_internal()
 * (obs-internal.h, no EXPORT; obs-source::ffi::source_preload_video_internal,
 * #5837), each the line of obs-source-preload-video-internal.c.h it stands
 * for. The caller rejects a NULL source and a NULL frame first. */
bool obs_source_preload_internal_destroying(const obs_source_t *source);
bool obs_source_preload_internal_destroying(const obs_source_t *source)
{
	return destroying(source);
}

/* frame_copy_shape(source->async_preload_frame) into *shape; false (and
 * *shape untouched) when there is no preload frame. */
bool obs_source_preload_internal_preload_shape(const obs_source_t *source, struct obs_source_frame_copy_shape *shape);
bool obs_source_preload_internal_preload_shape(const obs_source_t *source, struct obs_source_frame_copy_shape *shape)
{
	const struct obs_source_frame *preload = source->async_preload_frame;
	if (!preload)
		return false;
	shape->format = (int)preload->format;
	shape->width = preload->width;
	shape->height = preload->height;
	return true;
}

struct obs_source_frame_copy_shape obs_source_preload_internal_frame_shape(const struct obs_source_frame *frame);
struct obs_source_frame_copy_shape obs_source_preload_internal_frame_shape(const struct obs_source_frame *frame)
{
	const struct obs_source_frame_copy_shape shape = {
		.format = (int)frame->format,
		.width = frame->width,
		.height = frame->height,
	};
	return shape;
}

void obs_source_preload_internal_destroy_preload(obs_source_t *source);
void obs_source_preload_internal_destroy_preload(obs_source_t *source)
{
	obs_source_frame_destroy(source->async_preload_frame);
}

void obs_source_preload_internal_create_preload(obs_source_t *source, const struct obs_source_frame *frame);
void obs_source_preload_internal_create_preload(obs_source_t *source, const struct obs_source_frame *frame)
{
	source->async_preload_frame = obs_source_frame_create(frame->format, frame->width, frame->height);
}

void obs_source_preload_internal_copy(obs_source_t *source, const struct obs_source_frame *frame);
void obs_source_preload_internal_copy(obs_source_t *source, const struct obs_source_frame *frame)
{
	obs_source_frame_copy(source->async_preload_frame, frame);
}

void obs_source_preload_internal_lock_async(obs_source_t *source);
void obs_source_preload_internal_lock_async(obs_source_t *source)
{
	pthread_mutex_lock(&source->async_mutex);
}

void obs_source_preload_internal_set_last_frame_ts(obs_source_t *source, const struct obs_source_frame *frame);
void obs_source_preload_internal_set_last_frame_ts(obs_source_t *source, const struct obs_source_frame *frame)
{
	source->last_frame_ts = frame->timestamp;
}

void obs_source_preload_internal_unlock_async(obs_source_t *source);
void obs_source_preload_internal_unlock_async(obs_source_t *source)
{
	pthread_mutex_unlock(&source->async_mutex);
}
