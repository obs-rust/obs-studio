#include "libobs/build/libobs.pch.h"

size_t obs_output_caption_queue_slots(void);
struct deque *obs_output_caption_queue_track(obs_output_t *output, size_t index, void **mutex);

size_t obs_output_caption_queue_slots(void)
{
	return MAX_OUTPUT_VIDEO_ENCODERS;
}

struct deque *obs_output_caption_queue_track(obs_output_t *output, size_t index, void **mutex)
{
	struct caption_track_data *track = output->caption_tracks[index];
	*mutex = track ? &track->caption_mutex : NULL;
	return track ? &track->caption_data : NULL;
}
