#include "libobs/build/libobs.pch.h"

void obs_canvas_remove_set_flags(obs_canvas_t *canvas, uint32_t flags);

void obs_canvas_remove_set_flags(obs_canvas_t *canvas, uint32_t flags)
{
	canvas->flags = flags;
}
