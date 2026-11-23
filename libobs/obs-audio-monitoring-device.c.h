#include "libobs/build/libobs.pch.h"

void obs_reset_audio_monitoring(void)
{
	if (!obs_audio_monitoring_available())
		return;

	pthread_mutex_lock(&obs->audio.monitoring_mutex);

	for (size_t i = 0; i < obs->audio.monitors.num; i++) {
		struct audio_monitor *monitor = obs->audio.monitors.array[i];
		audio_monitor_reset(monitor);
	}

	pthread_mutex_unlock(&obs->audio.monitoring_mutex);
}

_Static_assert(OBS_CORE_SETUP_RULES_DO_NOT_SELF_MONITOR == OBS_SOURCE_DO_NOT_SELF_MONITOR,
	       "OBS_SOURCE_DO_NOT_SELF_MONITOR");

static bool check_all_aoc_sources(void *param, obs_source_t *src)
{
	UNUSED_PARAMETER(param);
	if (obs_core_setup_rules_aoc_check(src->info.output_flags)) {
		obs_data_t *settings = obs_source_get_settings(src);
		const char *device_id = obs_data_get_string(settings, "device_id");
		obs_source_audio_output_capture_device_changed(src, device_id);
		obs_data_release(settings);
	}
	return true;
}

bool obs_set_audio_monitoring_device(const char *name, const char *id)
{
	if (obs_core_setup_rules_monitor_missing(name != NULL) || obs_core_setup_rules_monitor_missing(id != NULL) ||
	    obs_core_setup_rules_monitor_missing(*name != '\0') || obs_core_setup_rules_monitor_missing(*id != '\0'))
		return false;

	if (obs_core_setup_rules_monitor_missing(obs_audio_monitoring_available()))
		return false;

	pthread_mutex_lock(&obs->audio.monitoring_mutex);

	if (obs_core_setup_rules_monitor_same(strcmp(id, obs->audio.monitoring_device_id))) {
		pthread_mutex_unlock(&obs->audio.monitoring_mutex);
		return true;
	}

	bfree(obs->audio.monitoring_device_name);
	bfree(obs->audio.monitoring_device_id);

	obs->audio.monitoring_device_name = bstrdup(name);
	obs->audio.monitoring_device_id = bstrdup(id);
	pthread_mutex_unlock(&obs->audio.monitoring_mutex);

	obs_reset_audio_monitoring();

	/* Check all Audio Output Capture sources for monitoring duplication. */
	obs_enum_sources(check_all_aoc_sources, NULL);

	return true;
}
