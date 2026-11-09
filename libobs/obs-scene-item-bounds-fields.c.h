#include "libobs/build/libobs.pch.h"

/* Preserve the scene-owned dependencies; standalone archive consumers resolve
 * these same real functions from libobs, never placeholder implementations. */
float obs_sceneitem_bounds_height_dependency(const obs_sceneitem_t *item);
void obs_sceneitem_bounds_update_dependency(obs_sceneitem_t *item);
float obs_sceneitem_bounds_canvas_height(const obs_sceneitem_t *item);
void obs_sceneitem_bounds_update(obs_sceneitem_t *item);

float obs_sceneitem_bounds_canvas_height(const obs_sceneitem_t *item)
{
	return obs_sceneitem_bounds_height_dependency(item);
}

void obs_sceneitem_bounds_update(obs_sceneitem_t *item)
{
	obs_sceneitem_bounds_update_dependency(item);
}
