#include "libobs/build/libobs.pch.h"

/* Rust validates NULL before any access, and the index before slot access. */
uint32_t obs_output_size_getters_flags(const obs_output_t *output);
uint32_t obs_output_size_getters_flags(const obs_output_t *output)
{
	return output->info.flags;
}
uint32_t obs_output_size_getters_scaled(const obs_output_t *output, bool height);
uint32_t obs_output_size_getters_scaled(const obs_output_t *output, bool height)
{
	return height ? output->scaled_height : output->scaled_width;
}
obs_encoder_t *obs_output_size_getters_encoder(const obs_output_t *output, size_t idx);
obs_encoder_t *obs_output_size_getters_encoder(const obs_output_t *output, size_t idx)
{
	return output->video_encoders[idx];
}
video_t *obs_output_size_getters_video(const obs_output_t *output);
video_t *obs_output_size_getters_video(const obs_output_t *output)
{
	return output->video;
}
const char *obs_output_size_getters_name(const obs_output_t *output);
const char *obs_output_size_getters_name(const obs_output_t *output)
{
	return output->context.name;
}
