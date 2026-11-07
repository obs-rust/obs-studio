#pragma once

#include "obs.h"

struct obs_core_video_mix;
struct obs_canvas_video_fields {
	struct obs_core_video_mix *const *mix;
};
extern struct obs_canvas_video_fields obs_canvas_video_fields(const obs_canvas_t *canvas);
extern video_t *const *obs_canvas_video_output_field(const struct obs_core_video_mix *mix);
