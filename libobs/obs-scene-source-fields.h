#pragma once

struct obs_scene;
struct obs_scene_item;
struct obs_source;

/* Borrow fields only from live, non-null handles. */
struct obs_source *const *obs_scene_source_fields(const struct obs_scene *scene);
struct obs_source *const *obs_sceneitem_source_fields(const struct obs_scene_item *item);
