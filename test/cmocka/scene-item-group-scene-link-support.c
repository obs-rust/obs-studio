/* Live private adapters for standalone Rust-static scene consumers. */
#include "obs-internal.h"
#include "obs-scene.h"

const bool *obs_sceneitem_group_scene_flag(const obs_sceneitem_t *item);
const bool *obs_sceneitem_group_scene_flag(const obs_sceneitem_t *item)
{
	return &item->is_group;
}

void *const *obs_sceneitem_group_scene_data(const obs_sceneitem_t *item);
void *const *obs_sceneitem_group_scene_data(const obs_sceneitem_t *item)
{
	return &item->source->context.data;
}
