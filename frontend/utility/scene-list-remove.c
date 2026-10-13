#include "scene-list-remove.h"

void scene_list_on_scene_removed(const struct scene_list_remove_ops *ops, void *scene)
{
	if (ops->take_from_list(ops->param, scene)) {
		ops->scene_list_changed(ops->param);
	}
}

void scene_list_remove_selected(const struct scene_list_remove_ops *ops, void *scene)
{
	/* obs_source_remove() reaches scene_list_on_scene_removed(), which
	 * already announced the change. */
	ops->remove_source(ops->param, scene);
}
