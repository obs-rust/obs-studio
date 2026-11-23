#include "libobs/build/libobs.pch.h"

void obs_add_raw_audio_callback(size_t mix_idx, const struct audio_convert_info *conversion,
				audio_output_callback_t callback, void *param)
{
	struct obs_core_audio *audio = &obs->audio;
	audio_output_connect(audio->audio, mix_idx, conversion, callback, param);
}
