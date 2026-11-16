#include "libobs/build/libobs.pch.h"

/* Keep the existing lock ordering in obs-scene.c.h. */
#define full_lock obs_scene_atomic_full_lock
#define full_unlock obs_scene_atomic_full_unlock

extern void obs_scene_atomic_full_lock(obs_scene_t *scene);
extern void obs_scene_atomic_full_unlock(obs_scene_t *scene);

void obs_scene_atomic_update(obs_scene_t *scene, obs_scene_atomic_update_func func, void *data)
{
	if (!scene)
		return;

	scene = obs_scene_get_ref(scene);
	if (!scene)
		return;

	full_lock(scene);
	func(data, scene);
	full_unlock(scene);
	obs_scene_release(scene);
}

#undef full_lock
#undef full_unlock
