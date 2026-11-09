#include "libobs/build/libobs.pch.h"

static void set_visibility(struct obs_scene_item *item, bool vis)
{
	pthread_mutex_lock(&item->actions_mutex);

	da_resize(item->audio_actions, 0);

	if (os_atomic_load_long(&item->active_refs) > 0) {
		if (!vis)
			obs_source_remove_active_child(item->parent->source, item->source);
	} else if (vis) {
		obs_source_add_active_child(item->parent->source, item->source);
	}

	os_atomic_set_long(&item->active_refs, vis ? 1 : 0);
	os_atomic_set_bool(&item->visible, vis);
	os_atomic_set_bool(&item->user_visible, vis);

	pthread_mutex_unlock(&item->actions_mutex);
}
