/* Original extracted C over real private layouts; valid C inputs only. */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#define obs_output_caption oracle_obs_output_caption
#include "obs-output-caption-queue.c.h"
#undef obs_output_caption
#include "output_caption_queue_fields.inc"

extern void obs_output_caption(obs_output_t *output, const struct obs_source_cea_708 *captions);

void oracle_output_caption_queue_layout(size_t *layout);
void oracle_output_caption_queue_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_source_cea_708);
	layout[1] = _Alignof(struct obs_source_cea_708);
	layout[2] = offsetof(struct obs_source_cea_708, data);
	layout[3] = offsetof(struct obs_source_cea_708, packets);
	layout[4] = offsetof(struct obs_source_cea_708, timestamp);
}

void *oracle_output_caption_queue_new(unsigned mask);
void *oracle_output_caption_queue_new(unsigned mask)
{
	obs_output_t *output = calloc(1, sizeof(*output));
	if (!output)
		abort();
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (!(mask & (1U << i)))
			continue;
		struct caption_track_data *track = calloc(1, sizeof(*track));
		if (!track || pthread_mutex_init(&track->caption_mutex, NULL))
			abort();
		deque_init(&track->caption_data);
		output->caption_tracks[i] = track;
	}
	return output;
}

void oracle_output_caption_queue_run(void *output, const uint8_t *data, uint32_t packets, bool rust);
void oracle_output_caption_queue_run(void *output, const uint8_t *data, uint32_t packets, bool rust)
{
	struct obs_source_cea_708 captions = {.data = data, .packets = packets, .timestamp = 12345};
	if (rust)
		obs_output_caption(output, &captions);
	else
		oracle_obs_output_caption(output, &captions);
}

size_t oracle_output_caption_queue_read(void *ptr, size_t index, uint8_t *data, size_t capacity);
size_t oracle_output_caption_queue_read(void *ptr, size_t index, uint8_t *data, size_t capacity)
{
	obs_output_t *output = ptr;
	struct caption_track_data *track = output->caption_tracks[index];
	if (!track)
		return 0;
	size_t size = track->caption_data.size;
	if (size && size <= capacity)
		deque_peek_front(&track->caption_data, data, size);
	return size;
}

void oracle_output_caption_queue_free(void *ptr);
void oracle_output_caption_queue_free(void *ptr)
{
	obs_output_t *output = ptr;
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		struct caption_track_data *track = output->caption_tracks[i];
		if (!track)
			continue;
		pthread_mutex_destroy(&track->caption_mutex);
		deque_free(&track->caption_data);
		free(track);
	}
	free(output);
}
