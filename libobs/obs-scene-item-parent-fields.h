#pragma once

struct obs_scene;
struct obs_scene_item;

/* Borrow the field only from a live, non-null item. */
struct obs_scene *const *obs_sceneitem_parent_fields(const struct obs_scene_item *item);
