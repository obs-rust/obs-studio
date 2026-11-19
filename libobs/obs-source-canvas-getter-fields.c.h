#include "libobs/build/libobs.pch.h"

obs_weak_canvas_t *obs_source_canvas_getter_weak(const obs_source_t *source);
obs_weak_canvas_t *obs_source_canvas_getter_weak(const obs_source_t *source)
{
	return source->canvas;
}
