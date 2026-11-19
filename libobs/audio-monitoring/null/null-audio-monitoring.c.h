#include "build/libobs-nix.pch.h"

/* The PulseAudio backend declares this private caller contract in its wrapper
 * header. The null backend has no wrapper header, but needs the same prototype. */
bool devices_match(const char *id1, const char *id2);

/* The canonical backend functions, defined only where this backend is
 * selected. Each forwards to its private adapter (null-audio-monitoring.h). */

bool obs_audio_monitoring_available(void)
{
	return obs_null_audio_monitoring_available();
}

void obs_enum_audio_monitoring_devices(obs_enum_audio_device_cb cb, void *data)
{
	obs_null_audio_monitoring_enum_devices(cb, data);
}

struct audio_monitor *audio_monitor_create(obs_source_t *source)
{
	return obs_null_audio_monitor_create(source);
}

void audio_monitor_reset(struct audio_monitor *monitor)
{
	obs_null_audio_monitor_reset(monitor);
}

void audio_monitor_destroy(struct audio_monitor *monitor)
{
	obs_null_audio_monitor_destroy(monitor);
}

bool devices_match(const char *id1, const char *id2)
{
	return obs_null_audio_monitoring_devices_match(id1, id2);
}
