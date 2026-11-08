#include "libobs/build/libobs.pch.h"

obs_source_t *obs_canvas_get_channel(obs_canvas_t *canvas, uint32_t channel)
{
	return obs_view_get_source(&canvas->view, channel);
}
