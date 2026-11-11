#include "libobs/build/libobs.pch.h"

void obs_remove_raw_audio_callback(size_t mix_idx, audio_output_callback_t callback, void *param)
{
	struct obs_core_audio *audio = &obs->audio;
	audio_output_disconnect(audio->audio, mix_idx, callback, param);
}
