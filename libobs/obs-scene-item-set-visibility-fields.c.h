#include "libobs/build/libobs.pch.h"

void obs_sceneitem_reconcile_lock(obs_sceneitem_t *item);
void obs_sceneitem_reconcile_unlock(obs_sceneitem_t *item);
void obs_sceneitem_reconcile_clear(obs_sceneitem_t *item);
long obs_sceneitem_reconcile_refs(obs_sceneitem_t *item);
void obs_sceneitem_reconcile_add(obs_sceneitem_t *item);
void obs_sceneitem_reconcile_remove(obs_sceneitem_t *item);
void obs_sceneitem_reconcile_normalize(obs_sceneitem_t *item, bool visible);

void obs_sceneitem_reconcile_lock(obs_sceneitem_t *item)
{
	pthread_mutex_lock(&item->actions_mutex);
}
void obs_sceneitem_reconcile_unlock(obs_sceneitem_t *item)
{
	pthread_mutex_unlock(&item->actions_mutex);
}
void obs_sceneitem_reconcile_clear(obs_sceneitem_t *item)
{
	da_resize(item->audio_actions, 0);
}
long obs_sceneitem_reconcile_refs(obs_sceneitem_t *item)
{
	return os_atomic_load_long(&item->active_refs);
}
void obs_sceneitem_reconcile_add(obs_sceneitem_t *item)
{
	obs_source_add_active_child(item->parent->source, item->source);
}
void obs_sceneitem_reconcile_remove(obs_sceneitem_t *item)
{
	obs_source_remove_active_child(item->parent->source, item->source);
}
void obs_sceneitem_reconcile_normalize(obs_sceneitem_t *item, bool visible)
{
	os_atomic_set_long(&item->active_refs, visible ? 1 : 0);
	os_atomic_set_bool(&item->visible, visible);
	os_atomic_set_bool(&item->user_visible, visible);
}
