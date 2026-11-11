#include "libobs/build/libobs.pch.h"

struct obs_output_set_media_fields {
	const uint32_t *flags;
	video_t **video;
	audio_t **audio;
	const char *name;
};

struct obs_output_set_media_fields obs_output_set_media_fields(obs_output_t *output);
struct obs_output_set_media_fields obs_output_set_media_fields(obs_output_t *output)
{
	return (struct obs_output_set_media_fields){&output->info.flags, &output->video, &output->audio,
						    output->context.name};
}
