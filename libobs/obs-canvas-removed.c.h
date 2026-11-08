#include "libobs/build/libobs-ported-service-getters.pch.h"

/* Private rule, shared with the canvas implementation. */
bool obs_canvas_rules_removed(uint32_t flags);

bool obs_canvas_removed(obs_canvas_t *canvas)
{
	return obs_canvas_rules_removed(canvas->flags);
}
