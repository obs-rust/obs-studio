#include "libobs/build/libobs.pch.h"

struct obs_canvas_channel_fields obs_canvas_channel_fields(obs_canvas_t *canvas)
{
	return (struct obs_canvas_channel_fields){&canvas->view, obs_view_get_source};
}
