#include "libobs/build/libobs.pch.h"

const struct obs_core *obs_video_info_query_core(void)
{
	return obs;
}

graphics_t *const *obs_video_info_query_graphics_field(const struct obs_core *core)
{
	return &core->video.graphics;
}

struct obs_core_video_mix *const *obs_video_info_query_mix_field(const struct obs_core *core)
{
	return &core->data.main_canvas->mix;
}

const struct obs_video_info *obs_video_info_query_value_field(const struct obs_core_video_mix *mix)
{
	return &mix->ovi;
}
