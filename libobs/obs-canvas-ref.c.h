#include "libobs/build/libobs-ported-service-getters.pch.h"

void obs_weak_canvas_addref(obs_weak_canvas_t *weak)
{
	if (!weak)
		return;

	obs_weak_ref_addref(&weak->ref);
}

void obs_weak_canvas_release(obs_weak_canvas_t *weak)
{
	if (!weak)
		return;

	if (obs_weak_ref_release(&weak->ref))
		bfree(weak);
}

obs_canvas_t *obs_canvas_get_ref(obs_canvas_t *canvas)
{
	if (!canvas)
		return NULL;

	return obs_weak_canvas_get_canvas((obs_weak_canvas_t *)canvas->context.control);
}

obs_weak_canvas_t *obs_canvas_get_weak_canvas(obs_canvas_t *canvas)
{
	if (!canvas)
		return NULL;

	obs_weak_canvas_t *weak = (obs_weak_canvas_t *)canvas->context.control;
	obs_weak_canvas_addref(weak);
	return weak;
}

obs_canvas_t *obs_weak_canvas_get_canvas(obs_weak_canvas_t *weak)
{
	if (!weak)
		return NULL;

	if (obs_weak_ref_get_ref(&weak->ref))
		return weak->canvas;

	return NULL;
}
