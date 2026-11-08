#include "libobs/build/libobs.pch.h"

const struct obs_core *obs_core_frame_interval_core(void)
{
	return obs;
}

const uint64_t *obs_core_frame_interval_field(const struct obs_core *core)
{
	return &core->video.video_frame_interval_ns;
}
