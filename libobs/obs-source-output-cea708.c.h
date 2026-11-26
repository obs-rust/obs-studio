#include "libobs/build/libobs.pch.h"

void obs_source_output_cea708(obs_source_t *source, const struct obs_source_cea_708 *captions)
{
	if (destroying(source))
		return;
	if (!captions) {
		return;
	}

	pthread_mutex_lock(&source->caption_cb_mutex);

	for (size_t i = source->caption_cb_list.num; i > 0; i--) {
		struct caption_cb_info info = source->caption_cb_list.array[i - 1];
		info.callback(info.param, source, captions);
	}

	pthread_mutex_unlock(&source->caption_cb_mutex);
}
