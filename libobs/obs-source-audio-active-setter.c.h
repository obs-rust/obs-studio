#include "libobs/build/libobs.pch.h"

void obs_source_set_audio_active(obs_source_t *source, bool active)
{
	if (!obs_source_valid(source, "obs_source_set_audio_active"))
		return;

	switch (obs_source_monitoring_audio_active(os_atomic_set_bool(&source->audio_active, active), active)) {
	case OBS_SOURCE_MONITORING_ACTIVE_ACTIVATE:
		obs_source_dosignal(source, "source_audio_activate", "audio_activate");
		break;
	case OBS_SOURCE_MONITORING_ACTIVE_DEACTIVATE:
		obs_source_dosignal(source, "source_audio_deactivate", "audio_deactivate");
		break;
	case OBS_SOURCE_MONITORING_ACTIVE_NONE:
		break;
	}
}
