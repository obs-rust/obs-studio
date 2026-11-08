#include "libobs/build/libobs.pch.h"

const char *obs_output_get_supported_video_codecs(const obs_output_t *output)
{
	return obs_output_valid(output, __func__) ? output->info.encoded_video_codecs : NULL;
}

const char *obs_output_get_supported_audio_codecs(const obs_output_t *output)
{
	return obs_output_valid(output, __func__) ? output->info.encoded_audio_codecs : NULL;
}
