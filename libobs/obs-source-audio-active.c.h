#include "libobs/build/libobs-ported-service-getters.pch.h"

bool obs_source_audio_active(const obs_source_t *source)
{
	return obs_source_valid(source, "obs_source_audio_active") ? os_atomic_load_bool(&source->audio_active) : false;
}
