/* Original getter and discovered adapter; view dependency is a local stand-in.
 * Production entry points are supplied only as callbacks (#3919). */
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include "obs-internal.h"

static obs_source_t *canvas_channel_view_get_source(obs_view_t *view, uint32_t channel)
{
	if (channel >= MAX_CHANNELS)
		return NULL;
	pthread_mutex_lock(&view->channels_mutex);
	obs_source_t *source = view->channels[channel];
	if (source) {
		struct obs_weak_source *control = (struct obs_weak_source *)source->context.control;
		if (!obs_weak_ref_get_ref(&control->ref))
			source = NULL;
	}
	pthread_mutex_unlock(&view->channels_mutex);
	return source;
}

obs_source_t *oracle_obs_canvas_get_channel(obs_canvas_t *canvas, uint32_t channel);

#define obs_view_get_source canvas_channel_view_get_source
#define obs_canvas_get_channel oracle_obs_canvas_get_channel
#include "obs-canvas-channel-getter.c.h"
#undef obs_canvas_get_channel
#include "canvas_channel_fields.inc"
#undef obs_view_get_source

void oracle_canvas_channel_layout(size_t *layout);
void oracle_canvas_channel_layout(size_t *layout)
{
	layout[0] = sizeof(struct obs_canvas_channel_fields);
	layout[1] = _Alignof(struct obs_canvas_channel_fields);
	layout[2] = offsetof(struct obs_canvas_channel_fields, view);
	layout[3] = offsetof(struct obs_canvas_channel_fields, get_source);
	layout[4] = sizeof(obs_view_t *);
	layout[5] = _Alignof(obs_view_t *);
	layout[6] = sizeof(((struct obs_canvas_channel_fields *)0)->get_source);
	layout[7] = _Alignof(obs_source_t * (*)(obs_view_t *, uint32_t));
}

bool oracle_canvas_channel_probe(uint32_t channel, bool present, bool expired, unsigned initial,
				 obs_source_t *(*getter)(obs_canvas_t *, uint32_t));
bool oracle_canvas_channel_probe(uint32_t channel, bool present, bool expired, unsigned initial,
				 obs_source_t *(*getter)(obs_canvas_t *, uint32_t))
{
	struct obs_canvas canvas = {0};
	struct obs_source source = {0};
	struct obs_weak_source control = {0};
	struct obs_canvas before;
	struct obs_source source_before;
	source.context.control = (struct obs_weak_object *)&control;
	control.source = &source;
	control.ref.refs = expired ? -1 : (long)initial;
	if (channel < MAX_CHANNELS && present)
		canvas.view.channels[channel] = &source;
	if (pthread_mutex_init(&canvas.view.channels_mutex, NULL) != 0)
		return false;
	memcpy(&before, &canvas, sizeof(canvas));
	memcpy(&source_before, &source, sizeof(source));
	obs_source_t *expected = channel < MAX_CHANNELS && present && !expired ? &source : NULL;
	obs_source_t *original = oracle_obs_canvas_get_channel(&canvas, channel);
	long original_refs = control.ref.refs;
	control.ref.refs = expired ? -1 : (long)initial;
	obs_source_t *ported = getter(&canvas, channel);
	bool equal = original == expected && ported == expected && control.ref.refs == original_refs &&
		     original_refs == (expired ? -1 : (long)initial + (expected != NULL)) &&
		     memcmp(&source, &source_before, sizeof(source)) == 0;
	/* pthread's internal lock bytes are not an API; compare all other state. */
	memcpy(&before.view.channels_mutex, &canvas.view.channels_mutex, sizeof(pthread_mutex_t));
	equal = equal && memcmp(&canvas, &before, sizeof(canvas)) == 0;
	int lock_result = pthread_mutex_trylock(&canvas.view.channels_mutex);
	equal = equal && lock_result == 0;
	if (lock_result == 0)
		pthread_mutex_unlock(&canvas.view.channels_mutex);
	pthread_mutex_destroy(&canvas.view.channels_mutex);
	return equal;
}
