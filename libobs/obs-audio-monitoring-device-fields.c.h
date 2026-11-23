#include "libobs/build/libobs.pch.h"

/* Private adapters for the Rust obs_set_audio_monitoring_device() and
 * obs_reset_audio_monitoring() (#5491). The monitoring fields, their mutex
 * and bmem copies, the monitor list and audio_monitor_reset(), the source
 * enumeration and the Audio Output Capture notice stay in C; Rust decides
 * which runs, and in what order. */

_Static_assert(OBS_CORE_SETUP_RULES_DO_NOT_SELF_MONITOR == OBS_SOURCE_DO_NOT_SELF_MONITOR,
	       "OBS_SOURCE_DO_NOT_SELF_MONITOR");

bool obs_audio_monitoring_device_available(void);
bool obs_audio_monitoring_device_available(void)
{
	return obs_audio_monitoring_available();
}

void obs_audio_monitoring_device_lock(void);
void obs_audio_monitoring_device_lock(void)
{
	pthread_mutex_lock(&obs->audio.monitoring_mutex);
}

void obs_audio_monitoring_device_unlock(void);
void obs_audio_monitoring_device_unlock(void)
{
	pthread_mutex_unlock(&obs->audio.monitoring_mutex);
}

int obs_audio_monitoring_device_compare(const char *id);
int obs_audio_monitoring_device_compare(const char *id)
{
	return strcmp(id, obs->audio.monitoring_device_id);
}

void obs_audio_monitoring_device_store(const char *name, const char *id);
void obs_audio_monitoring_device_store(const char *name, const char *id)
{
	bfree(obs->audio.monitoring_device_name);
	bfree(obs->audio.monitoring_device_id);

	obs->audio.monitoring_device_name = bstrdup(name);
	obs->audio.monitoring_device_id = bstrdup(id);
}

size_t obs_audio_monitoring_device_monitor_count(void);
size_t obs_audio_monitoring_device_monitor_count(void)
{
	return obs->audio.monitors.num;
}

void obs_audio_monitoring_device_reset_monitor(size_t index);
void obs_audio_monitoring_device_reset_monitor(size_t index)
{
	audio_monitor_reset(obs->audio.monitors.array[index]);
}

void obs_audio_monitoring_device_enum_sources(bool (*visit)(void *, obs_source_t *), void *param);
void obs_audio_monitoring_device_enum_sources(bool (*visit)(void *, obs_source_t *), void *param)
{
	obs_enum_sources(visit, param);
}

uint32_t obs_audio_monitoring_device_output_flags(const obs_source_t *source);
uint32_t obs_audio_monitoring_device_output_flags(const obs_source_t *source)
{
	return source->info.output_flags;
}

void obs_audio_monitoring_device_notify(obs_source_t *source);
void obs_audio_monitoring_device_notify(obs_source_t *source)
{
	obs_data_t *settings = obs_source_get_settings(source);
	const char *device_id = obs_data_get_string(settings, "device_id");
	obs_source_audio_output_capture_device_changed(source, device_id);
	obs_data_release(settings);
}
