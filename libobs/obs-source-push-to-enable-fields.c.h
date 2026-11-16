#include "libobs/build/libobs.pch.h"

struct obs_source_push_to_enable_fields obs_source_push_to_enable_fields(obs_source_t *source)
{
	return (struct obs_source_push_to_enable_fields){
		&source->audio_mutex,       &source->push_to_mute_enabled, &source->push_to_talk_enabled,
		&source->info.output_flags, &source->context.name,         obs_source_push_to_enable_signal_changed};
}
