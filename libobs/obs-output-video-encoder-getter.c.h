#include "libobs/build/libobs.pch.h"

obs_encoder_t *obs_output_get_video_encoder2(const obs_output_t *output, size_t idx)
{
	if (!obs_output_valid(output, "obs_output_get_video_encoder2"))
		return NULL;

	if (!obs_output_slots_video_slot(idx))
		return NULL;

	return output->video_encoders[idx];
}
