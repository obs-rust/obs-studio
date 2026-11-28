#include "libobs/build/libobs.pch.h"

/* obs_source_output_video_internal() (obs-internal.h, no EXPORT), the worker
 * every obs_source_output_video*() entry ends in, moved verbatim out of
 * obs-source.c.h (#5841). The async frame cache it fills and clears,
 * cache_video() and free_async_cache(), stays in obs-source.c.h, which comes
 * after this file in the unity group, so the two are no longer static:
 * obs-internal.h declares them (hidden, no EXPORT). */

void obs_source_output_video_internal(obs_source_t *source, const struct obs_source_frame *frame)
{
	if (!obs_source_valid(source, "obs_source_output_video"))
		return;

	/* the clock of the source's video, for its audio (#3747): taken alone,
	 * the graphics thread takes audio_buf_mutex inside async_mutex */
	pthread_mutex_lock(&source->audio_buf_mutex);
	source->last_output_frame_ts = frame ? frame->timestamp : 0;
	pthread_mutex_unlock(&source->audio_buf_mutex);

	if (!frame) {
		pthread_mutex_lock(&source->async_mutex);
		os_atomic_set_bool(&source->async_active, false);
		/* A new frame makes the source active again as soon as it is
		 * queued, but it only reaches the textures when it is rendered.
		 * Until then, do not draw what was there before. */
		os_atomic_set_bool(&source->async_textures_stale, true);
		source->last_frame_ts = 0;
		free_async_cache(source);
		pthread_mutex_unlock(&source->async_mutex);
		return;
	}

	source_profiler_async_frame_received(source);

	struct obs_source_frame *output = cache_video(source, frame);

	/* ------------------------------------------- */
	pthread_mutex_lock(&source->async_mutex);
	if (output) {
		if (os_atomic_dec_long(&output->refs) == 0) {
			obs_source_frame_destroy(output);
			output = NULL;
		} else {
			da_push_back(source->async_frames, &output);
			os_atomic_set_bool(&source->async_active, true);
		}
	}
	pthread_mutex_unlock(&source->async_mutex);
}
