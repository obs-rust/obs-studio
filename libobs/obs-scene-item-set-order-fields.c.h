#include "libobs/build/libobs.pch.h"

/* What obs_sceneitem_set_order() reads, writes and signals of a scene item
 * (the obs-scene shim). The caller rejects NULL, holds a reference to the
 * parent scene, and calls the read and link adapters only under the
 * scene's full lock (obs_scene_atomic_full_lock()). The parent and next
 * links come through obs_sceneitem_parent_fields() and
 * obs_scene_lookup_next(); the helpers themselves stay in obs-scene.c.h,
 * later in this same unity group. The C reference reaches them through
 * these adapters too (obs-scene-item-set-order-support.c.h). */
static inline void detach_sceneitem(struct obs_scene_item *item);
static inline void attach_sceneitem(struct obs_scene *parent, struct obs_scene_item *item, struct obs_scene_item *prev);
static inline void signal_reorder(struct obs_scene_item *item);

struct obs_scene_item *obs_sceneitem_set_order_prev(const struct obs_scene_item *item);
struct obs_scene_item *obs_sceneitem_set_order_prev(const struct obs_scene_item *item)
{
	return item->prev;
}

void obs_sceneitem_set_order_detach(struct obs_scene_item *item);
void obs_sceneitem_set_order_detach(struct obs_scene_item *item)
{
	detach_sceneitem(item);
}

void obs_sceneitem_set_order_attach(struct obs_scene *scene, struct obs_scene_item *item, struct obs_scene_item *prev);
void obs_sceneitem_set_order_attach(struct obs_scene *scene, struct obs_scene_item *item, struct obs_scene_item *prev)
{
	attach_sceneitem(scene, item, prev);
}

void obs_sceneitem_set_order_signal(struct obs_scene_item *item);
void obs_sceneitem_set_order_signal(struct obs_scene_item *item)
{
	signal_reorder(item);
}
