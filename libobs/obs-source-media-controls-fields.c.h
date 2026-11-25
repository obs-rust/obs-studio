#include "libobs/build/libobs.pch.h"

/* What the obs_source_media_* control calls read, queue and signal on a
 * source (the obs-source shims). The caller rejects NULL and keeps the source
 * alive; each read is one access, without a lock, as the C calls did. */
void *obs_source_media_controls_data(const obs_source_t *source);
void *obs_source_media_controls_data(const obs_source_t *source)
{
	return source->context.data;
}

uint32_t obs_source_media_controls_output_flags(const obs_source_t *source);
uint32_t obs_source_media_controls_output_flags(const obs_source_t *source)
{
	return source->info.output_flags;
}

/* Whether the callback of a queueing call is set; reads only that one. */
bool obs_source_media_controls_has_callback(const obs_source_t *source, int call);
bool obs_source_media_controls_has_callback(const obs_source_t *source, int call)
{
	switch (call) {
	case OBS_SOURCE_MEDIA_CALL_PLAY_PAUSE:
		return source->info.media_play_pause != NULL;
	case OBS_SOURCE_MEDIA_CALL_RESTART:
		return source->info.media_restart != NULL;
	case OBS_SOURCE_MEDIA_CALL_STOP:
		return source->info.media_stop != NULL;
	case OBS_SOURCE_MEDIA_CALL_NEXT:
		return source->info.media_next != NULL;
	case OBS_SOURCE_MEDIA_CALL_PREVIOUS:
		return source->info.media_previous != NULL;
	case OBS_SOURCE_MEDIA_CALL_SET_TIME:
		return source->info.media_set_time != NULL;
	default:
		return false;
	}
}

/* The queue push of the original calls, unchanged: one action appended
 * under media_actions_mutex. */
static inline void media_controls_push(obs_source_t *source, const struct media_action *action)
{
	pthread_mutex_lock(&source->media_actions_mutex);
	da_push_back(source->media_actions, action);
	pthread_mutex_unlock(&source->media_actions_mutex);
}

/* Restart, stop, next and previous: the type alone. */
void obs_source_media_controls_queue(obs_source_t *source, int type);
void obs_source_media_controls_queue(obs_source_t *source, int type)
{
	struct media_action action = {
		.type = (enum media_action_type)type,
	};

	media_controls_push(source, &action);
}

void obs_source_media_controls_queue_play_pause(obs_source_t *source, bool pause);
void obs_source_media_controls_queue_play_pause(obs_source_t *source, bool pause)
{
	struct media_action action = {
		.type = MEDIA_ACTION_PLAY_PAUSE,
		.pause = pause,
	};

	media_controls_push(source, &action);
}

void obs_source_media_controls_queue_set_time(obs_source_t *source, int64_t ms);
void obs_source_media_controls_queue_set_time(obs_source_t *source, int64_t ms)
{
	struct media_action action = {
		.type = MEDIA_ACTION_SET_TIME,
		.ms = ms,
	};

	media_controls_push(source, &action);
}

/* media_started and media_ended: the source's own handler only. */
void obs_source_media_controls_signal(obs_source_t *source, const char *signal);
void obs_source_media_controls_signal(obs_source_t *source, const char *signal)
{
	obs_source_dosignal(source, NULL, signal);
}
