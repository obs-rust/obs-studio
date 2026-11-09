#include "libobs/build/libobs.pch.h"

bool obs_canvas_signals_private(const obs_canvas_t *canvas)
{
	return canvas->context.private;
}
