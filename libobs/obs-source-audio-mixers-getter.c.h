#include "libobs/build/libobs.pch.h"

uint32_t obs_source_get_audio_mixers(const obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_get_audio_mixers"))
		return 0;
	if (!source->owns_info_id && !obs_source_monitoring_has_audio(source->info.output_flags))
		return 0;

	return source->audio_mixers;
}
