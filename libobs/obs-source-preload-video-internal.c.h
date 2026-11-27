#include "libobs/build/libobs.pch.h"

/* obs_source_preload_video_internal() (obs-internal.h, no EXPORT), the worker
 * every obs_source_preload_video*() entry ends in, with its static helpers
 * frame_copy_shape() and preload_frame_changed(), moved verbatim out of
 * obs-source.c.h (#5837). obs_source_set_video_frame_internal() stays in
 * obs-source.c.h and keeps its own copies of the two helpers, so these are
 * scoped here; the bodies keep the original names and the file also builds
 * in a translation unit of its own. */
#define frame_copy_shape preload_video_internal_frame_copy_shape
#define preload_frame_changed preload_video_internal_frame_changed

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

void obs_source_preload_video_internal(obs_source_t *source, const struct obs_source_frame *frame)
{
	if (!obs_source_valid(source, "obs_source_preload_video"))
		return;
	if (destroying(source))
		return;
	if (!frame)
		return;

	if (preload_frame_changed(source, frame)) {
		obs_source_frame_destroy(source->async_preload_frame);
		source->async_preload_frame = obs_source_frame_create(frame->format, frame->width, frame->height);
	}

	obs_source_frame_copy(source->async_preload_frame, frame);

	pthread_mutex_lock(&source->async_mutex);
	source->last_frame_ts = frame->timestamp;
	pthread_mutex_unlock(&source->async_mutex);
}

#undef preload_frame_changed
#undef frame_copy_shape
