#include "libobs/build/libobs.pch.h"

static bool enum_move_cb(obs_scene_t *scene, obs_sceneitem_t *item, void *param)
{
	UNUSED_PARAMETER(scene);

	obs_canvas_t *dst = param;
	obs_source_t *source = item->source;

	if (obs_source_is_group(source)) {
		obs_canvas_remove_source(source);
		obs_canvas_insert_source(dst, source);
	}

	return true;
}

void obs_canvas_move_scene(obs_scene_t *scene, obs_canvas_t *dst)
{
	obs_source_t *source = scene->source;
	obs_canvas_remove_source(source);
	obs_canvas_insert_source(dst, source);

	/* Also move all groups within this scene */
	obs_scene_enum_items(scene, enum_move_cb, dst);
}
