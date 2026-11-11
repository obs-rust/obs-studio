#include "libobs/build/libobs.pch.h"

char *const *obs_core_monitoring_device_name(const struct obs_core *core);
char *const *obs_core_monitoring_device_id(const struct obs_core *core);

char *const *obs_core_monitoring_device_name(const struct obs_core *core)
{
	return &core->audio.monitoring_device_name;
}

char *const *obs_core_monitoring_device_id(const struct obs_core *core)
{
	return &core->audio.monitoring_device_id;
}
