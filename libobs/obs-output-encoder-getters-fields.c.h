#include "libobs/build/libobs.pch.h"

/* Live adapter for obs_output_get_video_encoder() and
 * obs_output_get_audio_encoder() in rust/obs-output: one read per function.
 * Called only after Rust checks output (and the audio index). */
size_t obs_output_encoder_getters_video_slots(void);
size_t obs_output_encoder_getters_video_slots(void)
{
	return MAX_OUTPUT_VIDEO_ENCODERS;
}

bool obs_output_encoder_getters_video_occupied(const obs_output_t *output, size_t idx);
bool obs_output_encoder_getters_video_occupied(const obs_output_t *output, size_t idx)
{
	return output->video_encoders[idx] != NULL;
}

obs_encoder_t *obs_output_encoder_getters_audio_at(const obs_output_t *output, size_t idx);
obs_encoder_t *obs_output_encoder_getters_audio_at(const obs_output_t *output, size_t idx)
{
	return output->audio_encoders[idx];
}
