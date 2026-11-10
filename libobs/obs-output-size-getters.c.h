#include "libobs/build/libobs.pch.h"

/* Defined later by obs-output.c.h in the core-output unity group. */
static inline bool flag_encoded(const struct obs_output *output);
static inline bool log_flag_video(const struct obs_output *output, const char *func_name);

uint32_t obs_output_get_width2(const obs_output_t *output, size_t idx)
{
	if (!obs_output_valid(output, "obs_output_get_width2"))
		return 0;
	if (!log_flag_video(output, __func__))
		return 0;
	if (idx >= MAX_OUTPUT_VIDEO_ENCODERS)
		return 0;

	switch (obs_output_size_route(flag_encoded(output), output->video_encoders[idx] != NULL,
				      output->scaled_width)) {
	case OBS_OUTPUT_SIZE_ENCODER:
		return obs_encoder_get_width(output->video_encoders[idx]);
	case OBS_OUTPUT_SIZE_SCALED:
		return output->scaled_width;
	case OBS_OUTPUT_SIZE_VIDEO:
		return video_output_get_width(output->video);
	case OBS_OUTPUT_SIZE_ZERO:
	default:
		return 0;
	}
}

uint32_t obs_output_get_width(const obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_get_width"))
		return 0;
	if (!log_flag_video(output, __func__))
		return 0;

	return obs_output_get_width2(output, 0);
}

uint32_t obs_output_get_height2(const obs_output_t *output, size_t idx)
{
	if (!obs_output_valid(output, "obs_output_get_height2"))
		return 0;
	if (!log_flag_video(output, __func__))
		return 0;
	if (idx >= MAX_OUTPUT_VIDEO_ENCODERS)
		return 0;

	switch (obs_output_size_route(flag_encoded(output), output->video_encoders[idx] != NULL,
				      output->scaled_height)) {
	case OBS_OUTPUT_SIZE_ENCODER:
		return obs_encoder_get_height(output->video_encoders[idx]);
	case OBS_OUTPUT_SIZE_SCALED:
		return output->scaled_height;
	case OBS_OUTPUT_SIZE_VIDEO:
		return video_output_get_height(output->video);
	case OBS_OUTPUT_SIZE_ZERO:
	default:
		return 0;
	}
}

uint32_t obs_output_get_height(const obs_output_t *output)
{
	if (!obs_output_valid(output, "obs_output_get_height"))
		return 0;
	if (!log_flag_video(output, __func__))
		return 0;

	return obs_output_get_height2(output, 0);
}
