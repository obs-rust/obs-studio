#include "libobs/build/libobs.pch.h"

/* Atomic operations only: the render walk owns the scene mutex and the resize. */
void obs_sceneitem_group_resize_increment(obs_sceneitem_t *item);
long obs_sceneitem_group_resize_decrement(obs_sceneitem_t *item);
void obs_sceneitem_group_resize_schedule(obs_sceneitem_t *item);

void obs_sceneitem_group_resize_increment(obs_sceneitem_t *item)
{
	os_atomic_inc_long(&item->defer_group_resize);
}

long obs_sceneitem_group_resize_decrement(obs_sceneitem_t *item)
{
	return os_atomic_dec_long(&item->defer_group_resize);
}

void obs_sceneitem_group_resize_schedule(obs_sceneitem_t *item)
{
	os_atomic_set_bool(&item->update_group_resize, true);
}
