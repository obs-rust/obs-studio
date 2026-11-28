#include "libobs/build/libobs.pch.h"

static inline bool flag_encoded(const struct obs_output *output);
static bool get_first_audio_encoder_index(const struct obs_output *output, size_t *index);

audio_t *obs_output_audio(const obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_audio"))
		return NULL;

	if (!flag_encoded(output))
		return output->audio;

	size_t first_aenc_idx;
	if (get_first_audio_encoder_index(output, &first_aenc_idx))
		return obs_encoder_audio(output->audio_encoders[first_aenc_idx]);

	return NULL;
}
