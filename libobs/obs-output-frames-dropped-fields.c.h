#include "libobs/build/libobs.pch.h"

struct obs_output_frames_dropped_fields obs_output_frames_dropped_fields(const obs_output_t *output)
{
	return (struct obs_output_frames_dropped_fields){output->info.get_dropped_frames, output->context.data};
}
