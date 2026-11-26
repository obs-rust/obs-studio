#include "libobs/build/libobs.pch.h"

/* Private adapters of the Rust async-frame entry points
 * (obs_source_output_video(), _preload_video(), _set_video_frame() and
 * their *2() variants; obs-internal.h), each the call of
 * obs-source-frame-wrappers.c.h it stands for. */

bool obs_source_frame_wrappers_destroying(obs_source_t *source)
{
	return destroying(source);
}

void obs_source_frame_wrappers_output(obs_source_t *source, const struct obs_source_frame *frame)
{
	obs_source_output_video_internal(source, frame);
}

void obs_source_frame_wrappers_preload(obs_source_t *source, const struct obs_source_frame *frame)
{
	obs_source_preload_video_internal(source, frame);
}

void obs_source_frame_wrappers_set(obs_source_t *source, const struct obs_source_frame *frame)
{
	obs_source_set_video_frame_internal(source, frame);
}
