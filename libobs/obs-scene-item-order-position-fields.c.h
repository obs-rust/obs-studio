#include "libobs/build/libobs.pch.h"

/* Private adapter for the Rust obs_sceneitem_get_order_position(): the
 * parent list's head, read before the full lock as the C wrapper does. The
 * parent field (obs-scene-item-parent-fields.c.h) and the next links
 * (obs-scene-lookup-fields.c.h) come through their existing adapters. */
struct obs_scene_item *obs_scene_item_order_position_first(const struct obs_scene *scene);
struct obs_scene_item *obs_scene_item_order_position_first(const struct obs_scene *scene)
{
	return scene->first_item;
}
