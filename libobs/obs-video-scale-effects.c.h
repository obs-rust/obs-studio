#include "libobs/build/libobs.pch.h"

static inline uint32_t loaded_scale_effects(const struct obs_core_video *video)
{
	uint32_t loaded = 0;
	if (video->default_effect)
		loaded |= OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_DEFAULT);
	if (video->bilinear_lowres_effect)
		loaded |= OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES);
	if (video->bicubic_effect)
		loaded |= OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_BICUBIC);
	if (video->lanczos_effect)
		loaded |= OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_LANCZOS);
	if (video->area_effect)
		loaded |= OBS_VIDEO_SCALE_LOADED(OBS_VIDEO_SCALE_EFFECT_AREA);
	return loaded;
}

static inline gs_effect_t *scale_effect_pointer(const struct obs_core_video *video, enum obs_video_scale_effect effect)
{
	switch (effect) {
	case OBS_VIDEO_SCALE_EFFECT_BILINEAR_LOWRES:
		return video->bilinear_lowres_effect;
	case OBS_VIDEO_SCALE_EFFECT_BICUBIC:
		return video->bicubic_effect;
	case OBS_VIDEO_SCALE_EFFECT_LANCZOS:
		return video->lanczos_effect;
	case OBS_VIDEO_SCALE_EFFECT_AREA:
		return video->area_effect;
	case OBS_VIDEO_SCALE_EFFECT_DEFAULT:
	default:
		return video->default_effect;
	}
}
