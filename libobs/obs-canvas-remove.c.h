#include "libobs/build/libobs.pch.h"

void obs_canvas_remove(obs_canvas_t *canvas)
{
	/* Do not allow removing the main canvas, or canvases already marked as removed. */
	if (!obs_canvas_rules_removable(canvas->flags))
		return;

	obs_canvas_t *c = obs_canvas_get_ref(canvas);
	if (c) {
		c->flags = obs_canvas_rules_mark_removed(c->flags);
		canvas_dosignal(c, "canvas_remove", "remove");
		obs_canvas_release(c);
	}
}
