#include "libobs/build/libobs.pch.h"

/* Defined in obs-output.c.h later in the sorted core-output unity group. */
static inline bool flag_encoded(const struct obs_output *output);

video_t *obs_output_video(const obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_video"))
		return NULL;

	if (!flag_encoded(output))
		return output->video;

	obs_encoder_t *vencoder = obs_output_get_video_encoder(output);
	return obs_encoder_video(vencoder);
}
