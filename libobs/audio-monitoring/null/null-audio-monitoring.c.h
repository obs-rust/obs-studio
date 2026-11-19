#include "build/libobs-nix.pch.h"

/* The PulseAudio backend declares this private caller contract in its wrapper
 * header. The null backend has no wrapper header, but needs the same prototype. */
bool devices_match(const char *id1, const char *id2);

bool obs_audio_monitoring_available(void)
{
	return false;
}

void obs_enum_audio_monitoring_devices(obs_enum_audio_device_cb cb, void *data)
{
	UNUSED_PARAMETER(cb);
	UNUSED_PARAMETER(data);
}

struct audio_monitor *audio_monitor_create(obs_source_t *source)
{
	UNUSED_PARAMETER(source);
	return NULL;
}

void audio_monitor_reset(struct audio_monitor *monitor)
{
	UNUSED_PARAMETER(monitor);
}

void audio_monitor_destroy(struct audio_monitor *monitor)
{
	UNUSED_PARAMETER(monitor);
}

bool devices_match(const char *id1, const char *id2)
{
	UNUSED_PARAMETER(id1);
	UNUSED_PARAMETER(id2);
	return false;
}
