#include "libobs/build/libobs.pch.h"

/* One original operation per adapter; Rust owns reconciliation and ordering. */
void obs_sceneitem_visibility_lock(struct obs_scene_item *item);
void obs_sceneitem_visibility_unlock(struct obs_scene_item *item);
void obs_sceneitem_visibility_clear(struct obs_scene_item *item);
long obs_sceneitem_visibility_refs(struct obs_scene_item *item);
obs_source_t *obs_sceneitem_visibility_parent_source(struct obs_scene_item *item);
obs_source_t *obs_sceneitem_visibility_source(struct obs_scene_item *item);
void obs_sceneitem_visibility_set_refs(struct obs_scene_item *item, bool visible);
void obs_sceneitem_visibility_set_visible(struct obs_scene_item *item, bool visible);
void obs_sceneitem_visibility_set_user_visible(struct obs_scene_item *item, bool visible);

void obs_sceneitem_visibility_lock(struct obs_scene_item *item)
{
	pthread_mutex_lock(&item->actions_mutex);
}

void obs_sceneitem_visibility_unlock(struct obs_scene_item *item)
{
	pthread_mutex_unlock(&item->actions_mutex);
}

void obs_sceneitem_visibility_clear(struct obs_scene_item *item)
{
	da_resize(item->audio_actions, 0);
}

long obs_sceneitem_visibility_refs(struct obs_scene_item *item)
{
	return os_atomic_load_long(&item->active_refs);
}

obs_source_t *obs_sceneitem_visibility_parent_source(struct obs_scene_item *item)
{
	return item->parent->source;
}

obs_source_t *obs_sceneitem_visibility_source(struct obs_scene_item *item)
{
	return item->source;
}

void obs_sceneitem_visibility_set_refs(struct obs_scene_item *item, bool visible)
{
	os_atomic_set_long(&item->active_refs, visible ? 1 : 0);
}

void obs_sceneitem_visibility_set_visible(struct obs_scene_item *item, bool visible)
{
	os_atomic_set_bool(&item->visible, visible);
}

void obs_sceneitem_visibility_set_user_visible(struct obs_scene_item *item, bool visible)
{
	os_atomic_set_bool(&item->user_visible, visible);
}
