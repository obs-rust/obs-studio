#include "libobs/build/libobs.pch.h"

void obs_get_audio_monitoring_device(const char **name, const char **id)
{
	if (name)
		*name = obs->audio.monitoring_device_name;
	if (id)
		*id = obs->audio.monitoring_device_id;
}
