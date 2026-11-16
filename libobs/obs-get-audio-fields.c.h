#include "libobs/build/libobs.pch.h"

/* Read the same borrowed field as the original getter; no ownership change. */
struct obs_get_audio_fields {
	audio_t *audio;
};

struct obs_get_audio_fields obs_get_audio_fields(void);
struct obs_get_audio_fields obs_get_audio_fields(void)
{
	return (struct obs_get_audio_fields){obs->audio.audio};
}
