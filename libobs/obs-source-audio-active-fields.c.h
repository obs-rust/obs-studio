#include "libobs/build/libobs.pch.h"

const bool *obs_source_audio_active_field(const obs_source_t *source)
{
	return &source->audio_active;
}
