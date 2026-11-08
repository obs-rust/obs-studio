#pragma once

#include "obs.h"

struct obs_canvas_channel_fields {
	obs_view_t *view;
	obs_source_t *(*get_source)(obs_view_t *, uint32_t);
};
extern struct obs_canvas_channel_fields obs_canvas_channel_fields(obs_canvas_t *canvas);
