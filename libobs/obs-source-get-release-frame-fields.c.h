#include "libobs/build/libobs.pch.h"

/* Private adapters of the Rust obs_source_get_frame() and
 * obs_source_release_frame() (obs-internal.h), each the line of
 * obs-source-get-release-frame.c.h it stands for. NULL is rejected first. */

void obs_source_async_frame_lock(obs_source_t *source)
{
	pthread_mutex_lock(&source->async_mutex);
}

void obs_source_async_frame_unlock(obs_source_t *source)
{
	pthread_mutex_unlock(&source->async_mutex);
}

struct obs_source_frame *obs_source_async_frame_take_current(obs_source_t *source)
{
	struct obs_source_frame *frame = source->cur_async_frame;
	source->cur_async_frame = NULL;
	return frame;
}

void obs_source_async_frame_addref(struct obs_source_frame *frame)
{
	os_atomic_inc_long(&frame->refs);
}

bool obs_source_async_frame_release_ref(struct obs_source_frame *frame)
{
	return os_atomic_dec_long(&frame->refs) == 0;
}

void obs_source_async_frame_destroy(struct obs_source_frame *frame)
{
	obs_source_frame_destroy(frame);
}

void obs_source_async_frame_return(obs_source_t *source, struct obs_source_frame *frame)
{
	remove_async_frame(source, frame);
}

/* Moved unchanged from obs-source.c.h, so the adapters' closure is one
 * file for Rust-static consumers. */
void remove_async_frame(obs_source_t *source, struct obs_source_frame *frame)
{
	if (frame)
		frame->prev_frame = false;

	for (size_t i = 0; i < source->async_cache.num; i++) {
		struct async_frame *f = &source->async_cache.array[i];

		if (f->frame == frame) {
			f->used = false;
			break;
		}
	}
}
