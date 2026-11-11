#include "libobs/build/libobs.pch.h"

/* The scene item fields of the visibility audio ramp's item side, for its
 * Rust port (rust/obs-scene/src/ffi/item_audio_actions.rs), so struct
 * obs_scene_item is not mirrored in Rust. Each is the one operation
 * obs-scene-item-audio-actions.c.h made on the field: the lock and unlock
 * of actions_mutex, the address of the queue, the atomic reads and writes
 * of visible and active_refs, and the two sources of the child removal.
 * None of them takes another lock or calls into libobs. */
void obs_sceneitem_audio_actions_lock(struct obs_scene_item *item);
void obs_sceneitem_audio_actions_unlock(struct obs_scene_item *item);
obs_scene_item_action_array_t *obs_sceneitem_audio_actions_queue(struct obs_scene_item *item);
bool obs_sceneitem_audio_actions_visible(struct obs_scene_item *item);
void obs_sceneitem_audio_actions_set_visible(struct obs_scene_item *item, bool visible);
long obs_sceneitem_audio_actions_release(struct obs_scene_item *item);
obs_source_t *obs_sceneitem_audio_actions_parent_source(struct obs_scene_item *item);
obs_source_t *obs_sceneitem_audio_actions_source(struct obs_scene_item *item);

void obs_sceneitem_audio_actions_lock(struct obs_scene_item *item)
{
	pthread_mutex_lock(&item->actions_mutex);
}

void obs_sceneitem_audio_actions_unlock(struct obs_scene_item *item)
{
	pthread_mutex_unlock(&item->actions_mutex);
}

obs_scene_item_action_array_t *obs_sceneitem_audio_actions_queue(struct obs_scene_item *item)
{
	return &item->audio_actions;
}

bool obs_sceneitem_audio_actions_visible(struct obs_scene_item *item)
{
	return os_atomic_load_bool(&item->visible);
}

void obs_sceneitem_audio_actions_set_visible(struct obs_scene_item *item, bool visible)
{
	os_atomic_set_bool(&item->visible, visible);
}

long obs_sceneitem_audio_actions_release(struct obs_scene_item *item)
{
	return os_atomic_dec_long(&item->active_refs);
}

obs_source_t *obs_sceneitem_audio_actions_parent_source(struct obs_scene_item *item)
{
	return item->parent->source;
}

obs_source_t *obs_sceneitem_audio_actions_source(struct obs_scene_item *item)
{
	return item->source;
}
