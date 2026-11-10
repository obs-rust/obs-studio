#include "libobs/build/libobs.pch.h"

void obs_sceneitem_select_signal(obs_sceneitem_t *item, obs_scene_t *parent, bool selected);

/* The Rust core owns the decision and state write; C owns calldata and routing. */
void obs_sceneitem_select_signal(obs_sceneitem_t *item, obs_scene_t *parent, bool selected)
{
	struct calldata params;
	uint8_t stack[128];
	calldata_init_fixed(&params, stack, sizeof(stack));
	calldata_set_ptr(&params, "item", item);
	calldata_set_ptr(&params, "scene", parent);
	signal_handler_signal(parent->source->context.signals, selected ? "item_select" : "item_deselect", &params);
}
