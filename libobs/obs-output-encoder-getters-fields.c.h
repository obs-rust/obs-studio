#include "libobs/build/libobs.pch.h"

/* Called only after Rust validates output and idx. No private layout crosses the ABI. */
uint32_t obs_output_encoder_getters_video_mask(const obs_output_t *output);
uint32_t obs_output_encoder_getters_video_mask(const obs_output_t *output)
{
	uint32_t occupied = 0;
	for (size_t i = 0; i < MAX_OUTPUT_VIDEO_ENCODERS; i++) {
		if (output->video_encoders[i])
			occupied |= (uint32_t)1 << i;
	}
	return occupied;
}

obs_encoder_t *obs_output_encoder_getters_audio_at(const obs_output_t *output, size_t idx);
obs_encoder_t *obs_output_encoder_getters_audio_at(const obs_output_t *output, size_t idx)
{
	return output->audio_encoders[idx];
}
