#include "libobs/build/libobs.pch.h"

/* Defined in obs-scene.c.h in this same unity group. */
static void signal_parent(obs_scene_t *parent, const char *name, calldata_t *params);

void obs_sceneitem_select(obs_sceneitem_t *item, bool select)
{
	struct calldata params;
	uint8_t stack[128];
	const char *command = select ? "item_select" : "item_deselect";

	if (!item || item->selected == select || !item->parent)
		return;

	item->selected = select;

	calldata_init_fixed(&params, stack, sizeof(stack));
	calldata_set_ptr(&params, "item", item);

	signal_parent(item->parent, command, &params);
}
