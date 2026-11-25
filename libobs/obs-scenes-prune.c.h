#include "libobs/build/libobs.pch.h"

/* obs_scene_prune_sources, moved verbatim out of obs-scene.c.h (Refs #5731).
 * The file sorts after obs-scene.c.h in the core-source unity group, so the
 * scene item array typedef and the static walk are already declared; the
 * walk stays in obs-scene.c.h. The lock macros are file-local there, so this
 * file defines its own copies and undefines them at the end. */
#define video_lock(scene) pthread_mutex_lock(&scene->video_mutex)
#define video_unlock(scene) pthread_mutex_unlock(&scene->video_mutex)

static void update_transforms_and_prune_sources(obs_scene_t *scene, obs_scene_item_ptr_array_t *remove_items,
						obs_sceneitem_t *group_sceneitem, bool scene_size_changed);

void obs_scene_prune_sources(obs_scene_t *scene)
{
	obs_scene_item_ptr_array_t remove_items;
	da_init(remove_items);

	video_lock(scene);
	update_transforms_and_prune_sources(scene, &remove_items, NULL, false);
	video_unlock(scene);

	for (size_t i = 0; i < remove_items.num; i++)
		obs_sceneitem_release(remove_items.array[i]);

	da_free(remove_items);
}

/* File-local macros (BUILD.md). */
#undef video_lock
#undef video_unlock
