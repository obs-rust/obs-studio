#include "libobs/build/libobs.pch.h"

/* Shared unity-local helper defined in obs-output.c.h. */
static bool get_first_video_encoder_index(const struct obs_output *output, size_t *index);

obs_encoder_t *obs_output_get_video_encoder(const obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_get_video_encoder"))
		return NULL;

	size_t first_venc_idx;
	if (get_first_video_encoder_index(output, &first_venc_idx))
		return obs_output_get_video_encoder2(output, first_venc_idx);
	else
		return NULL;
}

obs_encoder_t *obs_output_get_audio_encoder(const obs_output_t *output, size_t idx)
{
	if (!obs_output_valid(output, "obs_output_get_audio_encoder"))
		return NULL;

	if (!obs_output_slots_audio_slot(idx))
		return NULL;

	return output->audio_encoders[idx];
}
