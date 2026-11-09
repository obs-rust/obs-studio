#include "libobs/build/libobs.pch.h"

struct obs_output_total_frames_fields obs_output_total_frames_fields(const obs_output_t *output)
{
	return (struct obs_output_total_frames_fields){output ? &output->total_frames : NULL};
}
