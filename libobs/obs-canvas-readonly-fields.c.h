#include "libobs/build/libobs.pch.h"

struct obs_canvas_readonly_fields obs_canvas_readonly_fields(const obs_canvas_t *canvas)
{
	return (struct obs_canvas_readonly_fields){&canvas->context.name, &canvas->context.uuid,
						   &canvas->context.signals, &canvas->mix};
}
