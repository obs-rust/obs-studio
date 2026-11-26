#include "libobs/build/libobs.pch.h"

/* For the isolated C reference only: the helpers the original
 * obs_sceneitem_set_order() calls are static in obs-scene.c.h, another unity
 * group, so they forward to the live non-EXPORT wrapper and adapters
 * (obs-scene.c.h, obs-scene-item-set-order-fields.c.h) that run the same
 * helpers there. */
void obs_scene_atomic_full_lock(obs_scene_t *scene);
void obs_scene_atomic_full_unlock(obs_scene_t *scene);
void obs_sceneitem_set_order_detach(struct obs_scene_item *item);
void obs_sceneitem_set_order_attach(struct obs_scene *scene, struct obs_scene_item *item, struct obs_scene_item *prev);
void obs_sceneitem_set_order_signal(struct obs_scene_item *item);

static inline void full_lock(struct obs_scene *scene)
{
	obs_scene_atomic_full_lock(scene);
}

static inline void full_unlock(struct obs_scene *scene)
{
	obs_scene_atomic_full_unlock(scene);
}

static inline void detach_sceneitem(struct obs_scene_item *item)
{
	obs_sceneitem_set_order_detach(item);
}

static inline void attach_sceneitem(struct obs_scene *parent, struct obs_scene_item *item, struct obs_scene_item *prev)
{
	obs_sceneitem_set_order_attach(parent, item, prev);
}

static inline void signal_reorder(struct obs_scene_item *item)
{
	obs_sceneitem_set_order_signal(item);
}
