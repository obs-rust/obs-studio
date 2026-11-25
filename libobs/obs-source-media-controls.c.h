#include "libobs/build/libobs.pch.h"

/* The original obs-source.c.h data_valid guard, scoped to this extraction. */
#define data_valid(source, f) (obs_source_valid(source, f) && (source)->context.data)

/* Whether a media call is accepted goes through
 * obs-source-media-admission.c, in the old order: the validity check (it
 * logs), then the output flags, then (in the caller, for the calls that
 * have one) the callback, each read only when the step before passed. */
static inline enum obs_source_media_admit media_admit(obs_source_t *source, bool valid, enum obs_source_media_call call)
{
	enum obs_source_media_admit admit = obs_source_media_admit(valid);
	if (admit == OBS_SOURCE_MEDIA_ADMIT_READ_FLAGS)
		admit = obs_source_media_admit_flags(call, source->info.output_flags);
	return admit;
}

void obs_source_media_play_pause(obs_source_t *source, bool pause)
{
	enum obs_source_media_admit admit = media_admit(source, data_valid(source, "obs_source_media_play_pause"),
							OBS_SOURCE_MEDIA_CALL_PLAY_PAUSE);
	if (admit == OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK)
		admit = obs_source_media_admit_callback(source->info.media_play_pause != NULL);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return;

	struct media_action action = {
		.type = MEDIA_ACTION_PLAY_PAUSE,
		.pause = pause,
	};

	pthread_mutex_lock(&source->media_actions_mutex);
	da_push_back(source->media_actions, &action);
	pthread_mutex_unlock(&source->media_actions_mutex);
}

void obs_source_media_restart(obs_source_t *source)
{
	enum obs_source_media_admit admit =
		media_admit(source, data_valid(source, "obs_source_media_restart"), OBS_SOURCE_MEDIA_CALL_RESTART);
	if (admit == OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK)
		admit = obs_source_media_admit_callback(source->info.media_restart != NULL);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return;

	struct media_action action = {
		.type = MEDIA_ACTION_RESTART,
	};

	pthread_mutex_lock(&source->media_actions_mutex);
	da_push_back(source->media_actions, &action);
	pthread_mutex_unlock(&source->media_actions_mutex);
}

void obs_source_media_stop(obs_source_t *source)
{
	enum obs_source_media_admit admit =
		media_admit(source, data_valid(source, "obs_source_media_stop"), OBS_SOURCE_MEDIA_CALL_STOP);
	if (admit == OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK)
		admit = obs_source_media_admit_callback(source->info.media_stop != NULL);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return;

	struct media_action action = {
		.type = MEDIA_ACTION_STOP,
	};

	pthread_mutex_lock(&source->media_actions_mutex);
	da_push_back(source->media_actions, &action);
	pthread_mutex_unlock(&source->media_actions_mutex);
}

void obs_source_media_next(obs_source_t *source)
{
	enum obs_source_media_admit admit =
		media_admit(source, data_valid(source, "obs_source_media_next"), OBS_SOURCE_MEDIA_CALL_NEXT);
	if (admit == OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK)
		admit = obs_source_media_admit_callback(source->info.media_next != NULL);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return;

	struct media_action action = {
		.type = MEDIA_ACTION_NEXT,
	};

	pthread_mutex_lock(&source->media_actions_mutex);
	da_push_back(source->media_actions, &action);
	pthread_mutex_unlock(&source->media_actions_mutex);
}

void obs_source_media_previous(obs_source_t *source)
{
	enum obs_source_media_admit admit =
		media_admit(source, data_valid(source, "obs_source_media_previous"), OBS_SOURCE_MEDIA_CALL_PREVIOUS);
	if (admit == OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK)
		admit = obs_source_media_admit_callback(source->info.media_previous != NULL);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return;

	struct media_action action = {
		.type = MEDIA_ACTION_PREVIOUS,
	};

	pthread_mutex_lock(&source->media_actions_mutex);
	da_push_back(source->media_actions, &action);
	pthread_mutex_unlock(&source->media_actions_mutex);
}

void obs_source_media_set_time(obs_source_t *source, int64_t ms)
{
	enum obs_source_media_admit admit =
		media_admit(source, data_valid(source, "obs_source_media_set_time"), OBS_SOURCE_MEDIA_CALL_SET_TIME);
	if (admit == OBS_SOURCE_MEDIA_ADMIT_READ_CALLBACK)
		admit = obs_source_media_admit_callback(source->info.media_set_time != NULL);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return;

	struct media_action action = {
		.type = MEDIA_ACTION_SET_TIME,
		.ms = ms,
	};

	pthread_mutex_lock(&source->media_actions_mutex);
	da_push_back(source->media_actions, &action);
	pthread_mutex_unlock(&source->media_actions_mutex);
}

void obs_source_media_started(obs_source_t *source)
{
	enum obs_source_media_admit admit = media_admit(source, obs_source_valid(source, "obs_source_media_started"),
							OBS_SOURCE_MEDIA_CALL_STARTED);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return;

	obs_source_dosignal(source, NULL, "media_started");
}

void obs_source_media_ended(obs_source_t *source)
{
	enum obs_source_media_admit admit =
		media_admit(source, obs_source_valid(source, "obs_source_media_ended"), OBS_SOURCE_MEDIA_CALL_ENDED);
	if (admit != OBS_SOURCE_MEDIA_ADMIT_ACCEPT)
		return;

	obs_source_dosignal(source, NULL, "media_ended");
}

#undef data_valid
