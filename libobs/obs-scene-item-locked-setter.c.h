#include "libobs/build/libobs.pch.h"

/* Shared with obs-scene.c.h in the core-source unity group. */
static void signal_parent(obs_scene_t *parent, const char *name, calldata_t *params);

bool obs_sceneitem_set_locked(obs_sceneitem_t *item, bool lock)
{
	struct calldata cd;
	uint8_t stack[256];

	if (!item)
		return false;

	if (item->locked == lock)
		return false;

	if (!item->parent)
		return false;

	item->locked = lock;

	calldata_init_fixed(&cd, stack, sizeof(stack));
	calldata_set_ptr(&cd, "item", item);
	calldata_set_bool(&cd, "locked", lock);

	signal_parent(item->parent, "item_locked", &cd);

	return true;
}
