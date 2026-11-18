#include "libobs/build/libobs.pch.h"

obs_canvas_t *obs_source_get_canvas(const obs_source_t *source)
{
	return obs_weak_canvas_get_canvas(source->canvas);
}
