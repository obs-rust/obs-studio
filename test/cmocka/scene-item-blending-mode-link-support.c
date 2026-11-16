/* Live adapters for standalone Rust-static scene consumers. */
#include "util/threading.h"
#include "obs-scene.h"

void *obs_sceneitem_blending_mode_field(obs_sceneitem_t *item);
void *obs_sceneitem_blending_mode_field(obs_sceneitem_t *item)
{
	return &item->blend_type;
}

void obs_sceneitem_blending_mode_dirty(obs_sceneitem_t *item);
void obs_sceneitem_blending_mode_dirty(obs_sceneitem_t *item)
{
	os_atomic_set_bool(&item->update_transform, true);
}
