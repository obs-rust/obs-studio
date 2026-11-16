#include "libobs/build/libobs.pch.h"

obs_weak_canvas_t *obs_canvas_ref_control(const obs_canvas_t *canvas)
{
	return (obs_weak_canvas_t *)canvas->context.control;
}
