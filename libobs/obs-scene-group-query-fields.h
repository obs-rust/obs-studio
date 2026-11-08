#pragma once

#include <stdbool.h>

struct obs_scene;
struct obs_scene_item;

/* Call only for a live non-null object, as under the original getter contract. */
const bool *obs_scene_group_query_fields(const struct obs_scene *scene);
const bool *obs_sceneitem_group_query_fields(const struct obs_scene_item *item);
