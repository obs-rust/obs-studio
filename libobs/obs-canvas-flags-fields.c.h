#include "libobs/build/libobs.pch.h"

struct obs_canvas_flags_fields obs_canvas_flags_fields(const obs_canvas_t *canvas)
{
	return (struct obs_canvas_flags_fields){&canvas->flags};
}
