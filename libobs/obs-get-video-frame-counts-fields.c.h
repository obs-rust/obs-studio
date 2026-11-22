#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_get_video_frame_counts(): the main
 * canvas's mix, the core's mixes_mutex, the mix list's length and entries,
 * the mix fields the frame-count rule asks about and each mix's video_t
 * counters. The list and the mix layout stay in C; Rust decides which reads
 * are made, and in which order. */
struct obs_core_video_mix *obs_get_video_frame_counts_main_mix(void);
struct obs_core_video_mix *obs_get_video_frame_counts_main_mix(void)
{
	return obs->data.main_canvas ? obs->data.main_canvas->mix : NULL;
}

void obs_get_video_frame_counts_lock(void);
void obs_get_video_frame_counts_lock(void)
{
	pthread_mutex_lock(&obs->video.mixes_mutex);
}

void obs_get_video_frame_counts_unlock(void);
void obs_get_video_frame_counts_unlock(void)
{
	pthread_mutex_unlock(&obs->video.mixes_mutex);
}

size_t obs_get_video_frame_counts_count(void);
size_t obs_get_video_frame_counts_count(void)
{
	return obs->video.mixes.num;
}

struct obs_core_video_mix *obs_get_video_frame_counts_at(size_t index);
struct obs_core_video_mix *obs_get_video_frame_counts_at(size_t index)
{
	return obs->video.mixes.array[index];
}

bool obs_get_video_frame_counts_encoder_only(const struct obs_core_video_mix *mix);
bool obs_get_video_frame_counts_encoder_only(const struct obs_core_video_mix *mix)
{
	return mix->encoder_only_mix;
}

bool obs_get_video_frame_counts_same_view(const struct obs_core_video_mix *mix,
					  const struct obs_core_video_mix *main_mix);
bool obs_get_video_frame_counts_same_view(const struct obs_core_video_mix *mix,
					  const struct obs_core_video_mix *main_mix)
{
	return mix->view == main_mix->view;
}

bool obs_get_video_frame_counts_has_video(const struct obs_core_video_mix *mix);
bool obs_get_video_frame_counts_has_video(const struct obs_core_video_mix *mix)
{
	return mix->video != NULL;
}

uint32_t obs_get_video_frame_counts_total(const struct obs_core_video_mix *mix);
uint32_t obs_get_video_frame_counts_total(const struct obs_core_video_mix *mix)
{
	return video_output_get_total_frames(mix->video);
}

uint32_t obs_get_video_frame_counts_skipped(const struct obs_core_video_mix *mix);
uint32_t obs_get_video_frame_counts_skipped(const struct obs_core_video_mix *mix)
{
	return video_output_get_skipped_frames(mix->video);
}
