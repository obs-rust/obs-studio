#include "libobs/build/libobs.pch.h"

/* Private adapters of the Rust per-source callback steps of
 * obs_source_video_tick() (obs-internal.h, no EXPORT;
 * obs-source::ffi::source_video_tick_callbacks, #5842), each the line of
 * obs-source-video-tick-callbacks.c.h it stands for. The step is 0 for
 * activate, 1 deactivate, 2 show, 3 hide. The caller passes a live source. */
static void (*obs_source_tick_callbacks_fn(const obs_source_t *source, int step))(void *data)
{
	switch (step) {
	case 0:
		return source->info.activate;
	case 1:
		return source->info.deactivate;
	case 2:
		return source->info.show;
	case 3:
		return source->info.hide;
	}
	return NULL;
}

bool obs_source_tick_callbacks_can_update(const obs_source_t *source);
bool obs_source_tick_callbacks_can_update(const obs_source_t *source)
{
	return source->context.data && source->info.update;
}

volatile long *obs_source_tick_callbacks_defer_count(obs_source_t *source);
volatile long *obs_source_tick_callbacks_defer_count(obs_source_t *source)
{
	return &source->defer_update_count;
}

void obs_source_tick_callbacks_update(obs_source_t *source);
void obs_source_tick_callbacks_update(obs_source_t *source)
{
	source->info.update(source->context.data, source->context.settings);
}

bool obs_source_tick_callbacks_has(const obs_source_t *source, int step);
bool obs_source_tick_callbacks_has(const obs_source_t *source, int step)
{
	return source->context.data && obs_source_tick_callbacks_fn(source, step);
}

void obs_source_tick_callbacks_call(obs_source_t *source, int step);
void obs_source_tick_callbacks_call(obs_source_t *source, int step)
{
	obs_source_tick_callbacks_fn(source, step)(source->context.data);
}

void obs_source_tick_callbacks_signal(obs_source_t *source, const char *signal_obs, const char *signal_source);
void obs_source_tick_callbacks_signal(obs_source_t *source, const char *signal_obs, const char *signal_source)
{
	obs_source_dosignal(source, signal_obs, signal_source);
}
