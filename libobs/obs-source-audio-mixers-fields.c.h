#include "libobs/build/libobs.pch.h"

bool obs_source_audio_mixers_owned_field(const obs_source_t *source);
bool obs_source_audio_mixers_owned_field(const obs_source_t *source)
{
	return source->owns_info_id;
}

uint32_t obs_source_audio_mixers_flags_field(const obs_source_t *source);
uint32_t obs_source_audio_mixers_flags_field(const obs_source_t *source)
{
	return source->info.output_flags;
}

uint32_t obs_source_audio_mixers_mask_field(const obs_source_t *source);
uint32_t obs_source_audio_mixers_mask_field(const obs_source_t *source)
{
	return source->audio_mixers;
}
