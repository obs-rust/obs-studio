#include "libobs/build/libobs.pch.h"

struct obs_canvas_video_fields obs_canvas_video_fields(const obs_canvas_t *canvas)
{
	return (struct obs_canvas_video_fields){&canvas->mix};
}

video_t *const *obs_canvas_video_output_field(const struct obs_core_video_mix *mix)
{
	return &mix->video;
}
