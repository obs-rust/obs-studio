#pragma once

#include <stdint.h>

struct obs_canvas;
struct obs_canvas_flags_fields {
	const uint32_t *flags;
};
extern struct obs_canvas_flags_fields obs_canvas_flags_fields(const struct obs_canvas *canvas);
