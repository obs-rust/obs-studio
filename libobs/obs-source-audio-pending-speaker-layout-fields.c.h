#include "libobs/build/libobs.pch.h"

/* The fields obs_source_audio_pending() and obs_source_get_speaker_layout()
 * read (the obs-source shims). The caller rejects NULL and keeps the source
 * alive; each call is one read. */
uint32_t obs_source_audio_pending_flags_field(const obs_source_t *source);
uint32_t obs_source_audio_pending_flags_field(const obs_source_t *source)
{
	return source->info.output_flags;
}

bool obs_source_audio_pending_field(const obs_source_t *source);
bool obs_source_audio_pending_field(const obs_source_t *source)
{
	return source->audio_pending;
}

_Static_assert(sizeof(enum speaker_layout) == sizeof(int), "enum speaker_layout crosses as int");
enum speaker_layout obs_source_speaker_layout_field(const obs_source_t *source);
enum speaker_layout obs_source_speaker_layout_field(const obs_source_t *source)
{
	return source->sample_info.speakers;
}
