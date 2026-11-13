#include "libobs/build/libobs.pch.h"

void obs_canvas_release(obs_canvas_t *canvas)
{
	if (!obs && canvas) {
		blog(LOG_WARNING, "Tried to release a canvas when the OBS core is shut down!");
		return;
	}

	if (!canvas)
		return;

	obs_weak_canvas_t *control = (obs_weak_canvas_t *)canvas->context.control;
	if (obs_ref_release(&control->ref)) {
		obs_canvas_destroy(canvas);
		obs_weak_canvas_release(control);
	}
}
