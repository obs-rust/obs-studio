#include "libobs/build/libobs.pch.h"

const char *obs_output_get_last_error(obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_get_last_error"))
		return NULL;

	if (output->last_error_message) {
		return output->last_error_message;
	} else {
		for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
			obs_encoder_t *vencoder = output->video_encoders[i];
			if (vencoder && vencoder->last_error_message) {
				return vencoder->last_error_message;
			}
		}

		for (size_t i = 0; i < MAX_OUTPUT_AUDIO_ENCODERS; i++) {
			obs_encoder_t *aencoder = output->audio_encoders[i];
			if (aencoder && aencoder->last_error_message) {
				return aencoder->last_error_message;
			}
		}
	}

	return NULL;
}
