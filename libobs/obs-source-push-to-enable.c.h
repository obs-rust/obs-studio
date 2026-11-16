#include "libobs/build/libobs.pch.h"

void obs_source_enable_push_to_mute(obs_source_t *source, bool enabled)
{
	if (!obs_source_valid(source, "obs_source_enable_push_to_mute"))
		return;

	pthread_mutex_lock(&source->audio_mutex);
	bool changed = obs_source_monitoring_changes(source->push_to_mute_enabled, enabled);
	if (obs_source_monitoring_push_to_logs(obs_source_get_output_flags(source), changed))
		blog(LOG_INFO, "source '%s' %s push-to-mute", obs_source_get_name(source),
		     enabled ? "enabled" : "disabled");

	source->push_to_mute_enabled = enabled;

	if (changed)
		obs_source_push_to_enable_signal_changed(source, "push_to_mute_changed", enabled);
	pthread_mutex_unlock(&source->audio_mutex);
}

void obs_source_enable_push_to_talk(obs_source_t *source, bool enabled)
{
	if (!obs_source_valid(source, "obs_source_enable_push_to_talk"))
		return;

	pthread_mutex_lock(&source->audio_mutex);
	bool changed = obs_source_monitoring_changes(source->push_to_talk_enabled, enabled);
	if (obs_source_monitoring_push_to_logs(obs_source_get_output_flags(source), changed))
		blog(LOG_INFO, "source '%s' %s push-to-talk", obs_source_get_name(source),
		     enabled ? "enabled" : "disabled");

	source->push_to_talk_enabled = enabled;

	if (changed)
		obs_source_push_to_enable_signal_changed(source, "push_to_talk_changed", enabled);
	pthread_mutex_unlock(&source->audio_mutex);
}
