#include "libobs/build/libobs.pch.h"

uint64_t obs_get_frame_interval_ns(void)
{
	return obs->video.video_frame_interval_ns;
}
