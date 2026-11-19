#include "libobs/build/libobs.pch.h"

/* Keep the original wrapper body; C owns the scene locks and shared walk. */
void obs_scene_atomic_full_lock(obs_scene_t *scene);
void obs_scene_atomic_full_unlock(obs_scene_t *scene);
obs_sceneitem_t *obs_sceneitem_parent_group_lookup(obs_scene_t *scene, obs_sceneitem_t *group_subitem);

#define full_lock obs_scene_atomic_full_lock
#define full_unlock obs_scene_atomic_full_unlock
#define get_sceneitem_parent_group obs_sceneitem_parent_group_lookup

obs_sceneitem_t *obs_sceneitem_get_group(obs_scene_t *scene, obs_sceneitem_t *group_subitem)
{
	if (!scene || !group_subitem || group_subitem->is_group)
		return NULL;

	full_lock(scene);
	obs_sceneitem_t *group = get_sceneitem_parent_group(scene, group_subitem);
	full_unlock(scene);

	return group;
}

#undef get_sceneitem_parent_group
#undef full_unlock
#undef full_lock
