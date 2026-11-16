#include "libobs/build/libobs.pch.h"

obs_source_t *obs_canvas_move_scene_source(const obs_scene_t *scene);
obs_source_t *obs_canvas_move_scene_source(const obs_scene_t *scene)
{
	return scene->source;
}

obs_source_t *obs_canvas_move_item_source(const obs_sceneitem_t *item);
obs_source_t *obs_canvas_move_item_source(const obs_sceneitem_t *item)
{
	return item->source;
}
