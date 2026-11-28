#include "libobs/build/libobs.pch.h"

/* Defined later in the same core-output unity group. */
static inline bool active(const struct obs_output *output);

static struct caption_text *caption_text_new(const char *text, size_t bytes, struct caption_text *tail,
					     struct caption_text **head, double display_duration)
{
	struct caption_text *next = bzalloc(sizeof(*next) + bytes + 1);
	memcpy(next->text, text, bytes);
	next->display_duration = display_duration;

	if (!*head)
		*head = next;
	else
		tail->next = next;

	return next;
}

void obs_output_output_caption_text2(obs_output_t *output, const char *text, double display_duration)
{
	if (!obs_output_valid(output, "obs_output_output_caption_text2"))
		return;
	if (!active(output))
		return;

	blog(LOG_DEBUG, "Caption text: %s", text);

	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		struct caption_track_data *ctrack = output->caption_tracks[i];
		const char *cursor = text;
		bool queued = false;

		if (!ctrack)
			continue;
		pthread_mutex_lock(&ctrack->caption_mutex);

		do {
			char page[CAPTION_PAGE_BYTES + 1];
			size_t bytes = obs_output_caption_text_page(&cursor, page);

			/* Preserve the old empty-string behavior, but do not append an
			 * empty page for trailing whitespace or invalid bytes. */
			if (bytes || !queued) {
				ctrack->caption_tail = caption_text_new(page, bytes, ctrack->caption_tail,
									&ctrack->caption_head, display_duration);
				queued = true;
			}
		} while (*cursor);

		pthread_mutex_unlock(&ctrack->caption_mutex);
	}
}
