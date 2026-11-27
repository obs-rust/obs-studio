#include "libobs/build/libobs.pch.h"

/* What obs_source_skip_video_filter() reads and draws (the obs-source
 * shim), each one step of the original: the filter's target and parent,
 * the parent's output flags, the target's video_render and deinterlace
 * mode, and the five draws. Two of the draws are static in obs-source.c.h
 * and reached through its hidden wrappers. Rust decides which run, and in
 * what order. */

void obs_source_skip_video_filter_main_render(obs_source_t *source);
void obs_source_skip_video_filter_async_video(obs_source_t *source);

obs_source_t *obs_source_skip_video_filter_get_target(obs_source_t *filter);
obs_source_t *obs_source_skip_video_filter_get_target(obs_source_t *filter)
{
	return obs_filter_get_target(filter);
}

obs_source_t *obs_source_skip_video_filter_get_parent(obs_source_t *filter);
obs_source_t *obs_source_skip_video_filter_get_parent(obs_source_t *filter)
{
	return obs_filter_get_parent(filter);
}

uint32_t obs_source_skip_video_filter_parent_flags(const obs_source_t *parent);
uint32_t obs_source_skip_video_filter_parent_flags(const obs_source_t *parent)
{
	return parent->info.output_flags;
}

bool obs_source_skip_video_filter_has_video_render(const obs_source_t *target);
bool obs_source_skip_video_filter_has_video_render(const obs_source_t *target)
{
	return target->info.video_render != NULL;
}

bool obs_source_skip_video_filter_deinterlacing(const obs_source_t *target);
bool obs_source_skip_video_filter_deinterlacing(const obs_source_t *target)
{
	return target->deinterlace_mode != OBS_DEINTERLACE_MODE_DISABLE;
}

void obs_source_skip_video_filter_draw_default(obs_source_t *target);
void obs_source_skip_video_filter_draw_default(obs_source_t *target)
{
	obs_source_default_render(target);
}

void obs_source_skip_video_filter_draw_main(obs_source_t *target);
void obs_source_skip_video_filter_draw_main(obs_source_t *target)
{
	obs_source_skip_video_filter_main_render(target);
}

void obs_source_skip_video_filter_draw_deinterlace(obs_source_t *target);
void obs_source_skip_video_filter_draw_deinterlace(obs_source_t *target)
{
	deinterlace_render(target);
}

void obs_source_skip_video_filter_draw_async(obs_source_t *target);
void obs_source_skip_video_filter_draw_async(obs_source_t *target)
{
	obs_source_skip_video_filter_async_video(target);
}

void obs_source_skip_video_filter_draw_target(obs_source_t *target);
void obs_source_skip_video_filter_draw_target(obs_source_t *target)
{
	obs_source_video_render(target);
}
