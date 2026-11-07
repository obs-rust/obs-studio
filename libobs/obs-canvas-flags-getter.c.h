#include "libobs/build/libobs-ported-service-getters.pch.h"

uint32_t obs_canvas_get_flags(const obs_canvas_t *canvas)
{
	return canvas->flags;
}
