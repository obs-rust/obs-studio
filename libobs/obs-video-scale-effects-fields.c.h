#include "libobs/build/libobs.pch.h"

/* C owns the real private layout. The output is five borrowed pointers. */
void obs_video_scale_effect_fields(const struct obs_core_video *video, gs_effect_t **effects);
void obs_video_scale_effect_fields(const struct obs_core_video *video, gs_effect_t **effects)
{
	effects[0] = video->default_effect;
	effects[1] = video->bilinear_lowres_effect;
	effects[2] = video->bicubic_effect;
	effects[3] = video->lanczos_effect;
	effects[4] = video->area_effect;
}
