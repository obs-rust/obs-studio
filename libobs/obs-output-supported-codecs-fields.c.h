#include "libobs/build/libobs.pch.h"

struct obs_output_supported_codecs_fields obs_output_supported_codecs_fields(const obs_output_t *output)
{
	return (struct obs_output_supported_codecs_fields){&output->info.encoded_video_codecs,
							   &output->info.encoded_audio_codecs};
}
