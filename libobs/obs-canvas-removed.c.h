#include "libobs/build/libobs.pch.h"

bool obs_canvas_removed(obs_canvas_t *canvas)
{
	return obs_canvas_rules_removed(canvas->flags);
}
