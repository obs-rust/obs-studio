#include "libobs/build/libobs.pch.h"

/* Called only after Rust validates output and idx. No private layout crosses the ABI. */
obs_encoder_t *obs_output_video_encoder_getter_at(const obs_output_t *output, size_t idx);
obs_encoder_t *obs_output_video_encoder_getter_at(const obs_output_t *output, size_t idx)
{
	return output->video_encoders[idx];
}
