#include "libobs/build/libobs.pch.h"

extern const struct obs_source_info group_info;

void obs_enum_sources(bool (*enum_proc)(void *, obs_source_t *), void *param)
{
	obs_source_t *source;

	pthread_mutex_lock(&obs->data.sources_mutex);
	source = obs->data.sources;

	while (source) {
		obs_source_t *s = obs_source_get_ref(source);
		if (s) {
			if (!s->context.private) {
				if (s->info.type == OBS_SOURCE_TYPE_INPUT && !enum_proc(param, s)) {
					obs_source_release(s);
					break;
				} else if (strcmp(s->info.id, group_info.id) == 0 && !enum_proc(param, s)) {
					obs_source_release(s);
					break;
				}
			}
			obs_source_release(s);
		}

		source = (obs_source_t *)source->context.hh_uuid.next;
	}

	pthread_mutex_unlock(&obs->data.sources_mutex);
}

void obs_canvas_enum_scenes(obs_canvas_t *canvas, bool (*enum_proc)(void *, obs_source_t *), void *param)
{
	obs_source_t *source;

	pthread_mutex_lock(&canvas->sources_mutex);

	source = canvas->sources;
	while (source) {
		obs_source_t *s = obs_source_get_ref(source);
		if (s) {
			if (source->info.type == OBS_SOURCE_TYPE_SCENE && !enum_proc(param, s)) {
				obs_source_release(s);
				break;
			}
			obs_source_release(s);
		}

		source = (obs_source_t *)source->context.hh.next;
	}

	pthread_mutex_unlock(&canvas->sources_mutex);
}

static inline void obs_enum_uuid(void *pstart, pthread_mutex_t *mutex, bool (*enum_proc)(void *, void *), void *param)
{
	struct obs_context_data **start = pstart, *context, *tmp;

	assert(start);
	assert(mutex);
	assert(enum_proc);

	pthread_mutex_lock(mutex);

	HASH_ITER (hh_uuid, *start, context, tmp) {
		if (!enum_proc(param, context))
			break;
	}

	pthread_mutex_unlock(mutex);
}

void obs_enum_all_sources(bool (*enum_proc)(void *, obs_source_t *), void *param)
{
	obs_enum_uuid(&obs->data.sources, &obs->data.sources_mutex, (bool (*)(void *, void *))enum_proc, param);
}

void obs_enum_canvases(bool (*enum_proc)(void *, obs_canvas_t *), void *param)
{
	struct obs_context_data *start = (struct obs_context_data *)obs->data.named_canvases;
	struct obs_context_data *context, *tmp;

	pthread_mutex_lock(&obs->data.canvases_mutex);

	HASH_ITER (hh, start, context, tmp) {
		obs_canvas_t *canvas = (obs_canvas_t *)context;
		if (!enum_proc(param, canvas))
			break;
	}

	pthread_mutex_unlock(&obs->data.canvases_mutex);
}
