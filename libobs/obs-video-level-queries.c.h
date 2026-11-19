#include "libobs/build/libobs.pch.h"

/* With no core, the levels are the no-video defaults (#2566). */
float obs_get_video_sdr_white_level(void)
{
	if (!obs)
		return 300.f;

	struct obs_core_video *video = &obs->video;
	return video->graphics ? video->sdr_white_level : 300.f;
}

float obs_get_video_hdr_nominal_peak_level(void)
{
	if (!obs)
		return 1000.f;

	struct obs_core_video *video = &obs->video;
	return video->graphics ? video->hdr_nominal_peak_level : 1000.f;
}
