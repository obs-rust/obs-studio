#include "libobs/build/libobs-ported-null-audio-monitoring.pch.h"

/* The original null backend, for the C reference and the Tier 3 oracle. Its
 * bodies are the C's, verbatim, under the private adapter names
 * (null-audio-monitoring.h) the canonical forwarders call. */

bool obs_null_audio_monitoring_available(void)
{
	return false;
}

void obs_null_audio_monitoring_enum_devices(bool (*cb)(void *data, const char *name, const char *id), void *data)
{
	UNUSED_PARAMETER(cb);
	UNUSED_PARAMETER(data);
}

struct audio_monitor *obs_null_audio_monitor_create(struct obs_source *source)
{
	UNUSED_PARAMETER(source);
	return NULL;
}

void obs_null_audio_monitor_reset(struct audio_monitor *monitor)
{
	UNUSED_PARAMETER(monitor);
}

void obs_null_audio_monitor_destroy(struct audio_monitor *monitor)
{
	UNUSED_PARAMETER(monitor);
}

bool obs_null_audio_monitoring_devices_match(const char *id1, const char *id2)
{
	UNUSED_PARAMETER(id1);
	UNUSED_PARAMETER(id2);
	return false;
}
