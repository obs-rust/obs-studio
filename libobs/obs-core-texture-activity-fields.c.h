#include "libobs/build/libobs.pch.h"

const struct obs_core *obs_core_texture_activity_core(void)
{
	return obs;
}

obs_canvas_t *const *obs_core_texture_activity_canvas(const struct obs_core *core)
{
	return &core->data.main_canvas;
}

struct obs_core_video_mix *const *obs_core_texture_activity_mix(const obs_canvas_t *canvas)
{
	return &canvas->mix;
}

struct obs_core_texture_activity_fields obs_core_texture_activity_fields(const struct obs_core_video_mix *mix)
{
	return (struct obs_core_texture_activity_fields){&mix->using_nv12_tex, &mix->using_p010_tex};
}
