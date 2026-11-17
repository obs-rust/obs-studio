#include "libobs/build/libobs.pch.h"

/* Live adapter for the scene item bounds setters, which are Rust in the
 * hybrid build (obs-scene::item_bounds_setters) and C in their own ported
 * group. It wraps the two scene statics they call, defined later in this
 * unity group, and gives obs-scene.c.h's loaders the shared bounds type
 * check, which lives with the setters. */
static inline void size_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
static void scene_item_bounds_update_transform(obs_sceneitem_t *item);

enum obs_bounds_type obs_scene_bounds_valid_type(int64_t type);
void obs_sceneitem_bounds_size_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item);
void obs_sceneitem_bounds_update_transform(obs_sceneitem_t *item);

static inline enum obs_bounds_type valid_bounds_type(int64_t type)
{
	return obs_scene_bounds_valid_type(type);
}

void obs_sceneitem_bounds_size_from_absolute(struct vec2 *dst, const struct vec2 *v, const obs_sceneitem_t *item)
{
	size_from_absolute(dst, v, item);
}

void obs_sceneitem_bounds_update_transform(obs_sceneitem_t *item)
{
	scene_item_bounds_update_transform(item);
}
