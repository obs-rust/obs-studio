#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_enum_sources(), obs_enum_all_sources(),
 * obs_enum_canvases() and obs_canvas_enum_scenes() (#5495): the mutexes,
 * the table heads and links, the source reference calls and the source
 * fields they read. The tables, their layout and the reference counting stay
 * in C; Rust decides which reads and calls are made, and in which order. */

extern const struct obs_source_info group_info;

/* The core's source table (uuid order). */
void obs_enum_sources_lock(void);
void obs_enum_sources_lock(void)
{
	pthread_mutex_lock(&obs->data.sources_mutex);
}

void obs_enum_sources_unlock(void);
void obs_enum_sources_unlock(void)
{
	pthread_mutex_unlock(&obs->data.sources_mutex);
}

obs_source_t *obs_enum_sources_first(void);
obs_source_t *obs_enum_sources_first(void)
{
	return obs->data.sources;
}

obs_source_t *obs_enum_sources_next(const obs_source_t *source);
obs_source_t *obs_enum_sources_next(const obs_source_t *source)
{
	return (obs_source_t *)source->context.hh_uuid.next;
}

obs_source_t *obs_enum_sources_get_ref(obs_source_t *source);
obs_source_t *obs_enum_sources_get_ref(obs_source_t *source)
{
	return obs_source_get_ref(source);
}

void obs_enum_sources_release(obs_source_t *source);
void obs_enum_sources_release(obs_source_t *source)
{
	obs_source_release(source);
}

bool obs_enum_sources_private(const obs_source_t *source);
bool obs_enum_sources_private(const obs_source_t *source)
{
	return source->context.private;
}

bool obs_enum_sources_input(const obs_source_t *source);
bool obs_enum_sources_input(const obs_source_t *source)
{
	return source->info.type == OBS_SOURCE_TYPE_INPUT;
}

bool obs_enum_sources_group(const obs_source_t *source);
bool obs_enum_sources_group(const obs_source_t *source)
{
	return strcmp(source->info.id, group_info.id) == 0;
}

bool obs_enum_sources_scene(const obs_source_t *source);
bool obs_enum_sources_scene(const obs_source_t *source)
{
	return source->info.type == OBS_SOURCE_TYPE_SCENE;
}

/* A canvas's own sources (name order). */
void obs_enum_sources_canvas_lock(obs_canvas_t *canvas);
void obs_enum_sources_canvas_lock(obs_canvas_t *canvas)
{
	pthread_mutex_lock(&canvas->sources_mutex);
}

void obs_enum_sources_canvas_unlock(obs_canvas_t *canvas);
void obs_enum_sources_canvas_unlock(obs_canvas_t *canvas)
{
	pthread_mutex_unlock(&canvas->sources_mutex);
}

obs_source_t *obs_enum_sources_canvas_first(const obs_canvas_t *canvas);
obs_source_t *obs_enum_sources_canvas_first(const obs_canvas_t *canvas)
{
	return canvas->sources;
}

obs_source_t *obs_enum_sources_canvas_next(const obs_source_t *source);
obs_source_t *obs_enum_sources_canvas_next(const obs_source_t *source)
{
	return (obs_source_t *)source->context.hh.next;
}

/* The core's named canvases (name order). */
void obs_enum_sources_canvases_lock(void);
void obs_enum_sources_canvases_lock(void)
{
	pthread_mutex_lock(&obs->data.canvases_mutex);
}

void obs_enum_sources_canvases_unlock(void);
void obs_enum_sources_canvases_unlock(void)
{
	pthread_mutex_unlock(&obs->data.canvases_mutex);
}

obs_canvas_t *obs_enum_sources_canvases_first(void);
obs_canvas_t *obs_enum_sources_canvases_first(void)
{
	return obs->data.named_canvases;
}

obs_canvas_t *obs_enum_sources_canvases_next(const obs_canvas_t *canvas);
obs_canvas_t *obs_enum_sources_canvases_next(const obs_canvas_t *canvas)
{
	return (obs_canvas_t *)canvas->context.hh.next;
}
