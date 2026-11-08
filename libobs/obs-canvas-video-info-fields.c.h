#include "libobs/build/libobs.pch.h"

graphics_t *const *obs_canvas_video_info_graphics_field(void)
{
	return &obs->video.graphics;
}

struct obs_core_video_mix *const *obs_canvas_video_info_mix_field(const obs_canvas_t *canvas)
{
	return &canvas->mix;
}

const struct obs_video_info *obs_canvas_video_info_value_field(const struct obs_core_video_mix *mix)
{
	return &mix->ovi;
}
