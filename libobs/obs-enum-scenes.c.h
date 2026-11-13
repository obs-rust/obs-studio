#include "libobs/build/libobs.pch.h"

void obs_enum_scenes(bool (*enum_proc)(void *, obs_source_t *), void *param)
{
	obs_canvas_enum_scenes(obs->data.main_canvas, enum_proc, param);
}
