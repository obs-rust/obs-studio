#include "libobs/build/libobs.pch.h"

/* Keep the existing calldata/observer primitive shared with unported sources. */
void obs_source_audio_active_setter_signal(obs_source_t *source, const char *global, const char *local);
void obs_source_audio_active_setter_signal(obs_source_t *source, const char *global, const char *local)
{
	obs_source_dosignal(source, global, local);
}
