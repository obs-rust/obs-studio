#include "libobs/build/libobs.pch.h"

/* Keep the file-local output flag predicates of obs-source.c.h available when
 * compiled independently. */
static inline bool audio_pending_is_audio_source(const struct obs_source *source)
{
	return source->info.output_flags & OBS_SOURCE_AUDIO;
}

static inline bool audio_pending_is_composite_source(const struct obs_source *source)
{
	return source->info.output_flags & OBS_SOURCE_COMPOSITE;
}

bool obs_source_audio_pending(const obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_audio_pending"))
		return true;

	if (obs_source_removed(source))
		return true;

	return (audio_pending_is_composite_source(source) || audio_pending_is_audio_source(source))
		       ? source->audio_pending
		       : true;
}

enum speaker_layout obs_source_get_speaker_layout(obs_source_t *source)
{
	if (!obs_source_valid(source, "obs_source_get_audio_channels"))
		return SPEAKERS_UNKNOWN;

	return source->sample_info.speakers;
}
