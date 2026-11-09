#include "libobs/build/libobs.pch.h"

static inline uint32_t loaded_scale_effects(const struct obs_core_video *video);
static inline gs_effect_t *scale_effect_pointer(const struct obs_core_video *video, enum obs_video_scale_effect effect);

uint32_t obs_video_loaded_scale_effects(const struct obs_core_video *video);
gs_effect_t *obs_video_scale_effect_pointer(const struct obs_core_video *video, enum obs_video_scale_effect effect);

uint32_t obs_video_loaded_scale_effects(const struct obs_core_video *video)
{
    return loaded_scale_effects(video);
}

gs_effect_t *obs_video_scale_effect_pointer(const struct obs_core_video *video, enum obs_video_scale_effect effect)
{
    return scale_effect_pointer(video, effect);
}
