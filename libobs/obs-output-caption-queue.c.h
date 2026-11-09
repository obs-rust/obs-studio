#include "libobs/build/libobs-ported-service-getters.pch.h"

void obs_output_caption(obs_output_t *output, const struct obs_source_cea_708 *captions)
{
	for (int i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		struct caption_track_data *ctrack = output->caption_tracks[i];
		if (!ctrack) {
			continue;
		}
		pthread_mutex_lock(&ctrack->caption_mutex);
		for (size_t pkt = 0; pkt < captions->packets; pkt++) {
			deque_push_back(&ctrack->caption_data, captions->data + (pkt * 3), 3 * sizeof(uint8_t));
		}
		pthread_mutex_unlock(&ctrack->caption_mutex);
	}
}
