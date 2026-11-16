#include "libobs/build/libobs.pch.h"

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
