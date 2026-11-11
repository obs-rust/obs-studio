#include "libobs/build/libobs-ported-service-getters.pch.h"

void obs_canvas_render(obs_canvas_t *canvas)
{
	obs_view_render(&canvas->view);
}
