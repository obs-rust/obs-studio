#include "libobs/build/libobs.pch.h"

/* What obs_source_set_flags() and obs_source_set_audio_mixers() read, write
 * and signal on a source (the obs-source shims). The caller rejects NULL and
 * keeps the source alive; each call is one access, without a lock, as the C
 * setters did. */
uint32_t obs_source_flags_mixers_flags(const obs_source_t *source);
uint32_t obs_source_flags_mixers_flags(const obs_source_t *source)
{
	return source->flags;
}

void obs_source_flags_mixers_store_flags(obs_source_t *source, uint32_t flags);
void obs_source_flags_mixers_store_flags(obs_source_t *source, uint32_t flags)
{
	source->flags = flags;
}

bool obs_source_flags_mixers_owns_info_id(const obs_source_t *source);
bool obs_source_flags_mixers_owns_info_id(const obs_source_t *source)
{
	return source->owns_info_id;
}

uint32_t obs_source_flags_mixers_output_flags(const obs_source_t *source);
uint32_t obs_source_flags_mixers_output_flags(const obs_source_t *source)
{
	return source->info.output_flags;
}

uint32_t obs_source_flags_mixers_audio_mixers(const obs_source_t *source);
uint32_t obs_source_flags_mixers_audio_mixers(const obs_source_t *source)
{
	return source->audio_mixers;
}

void obs_source_flags_mixers_store_audio_mixers(obs_source_t *source, uint32_t mixers);
void obs_source_flags_mixers_store_audio_mixers(obs_source_t *source, uint32_t mixers)
{
	source->audio_mixers = mixers;
}

signal_handler_t *obs_source_flags_mixers_signals(const obs_source_t *source);
signal_handler_t *obs_source_flags_mixers_signals(const obs_source_t *source)
{
	return source->context.signals;
}
