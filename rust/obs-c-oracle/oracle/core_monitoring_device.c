/* Original C getter over an independent fixture with the real private layout. */
#include <stddef.h>
#define obs monitoring_device_oracle_core
#define obs_get_audio_monitoring_device oracle_obs_get_audio_monitoring_device
#include "obs-core-monitoring-device.c.h"
#undef obs_get_audio_monitoring_device
#include "core_monitoring_device_fields.inc"
#undef obs

static struct obs_core monitoring_device_state;
struct obs_core *monitoring_device_oracle_core = &monitoring_device_state;

void *oracle_core_monitoring_device_set(char *name, char *id);
void *oracle_core_monitoring_device_set(char *name, char *id)
{
	monitoring_device_state.audio.monitoring_device_name = name;
	monitoring_device_state.audio.monitoring_device_id = id;
	return &monitoring_device_state;
}
