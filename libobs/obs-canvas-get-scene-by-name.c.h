#include "libobs/build/libobs.pch.h"

obs_scene_t *obs_canvas_get_scene_by_name(obs_canvas_t *canvas, const char *name)
{
	obs_source_t *source = obs_canvas_get_source_by_name(canvas, name);
	obs_scene_t *scene = obs_scene_from_source(source);
	if (!scene) {
		obs_source_release(source);
		return NULL;
	}
	return scene;
}
