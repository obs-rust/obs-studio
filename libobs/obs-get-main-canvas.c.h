#include "libobs/build/libobs.pch.h"

obs_canvas_t *obs_get_main_canvas(void)
{
	return obs_canvas_get_ref(obs->data.main_canvas);
}
