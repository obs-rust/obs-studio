#pragma once

#include "obs.h"

struct obs_core_video_mix;
extern graphics_t *const *obs_canvas_video_info_graphics_field(void);
extern struct obs_core_video_mix *const *obs_canvas_video_info_mix_field(const obs_canvas_t *canvas);
extern const struct obs_video_info *obs_canvas_video_info_value_field(const struct obs_core_video_mix *mix);
