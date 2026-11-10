#include "libobs/build/libobs.pch.h"

void obs_sceneitem_locked_signal(obs_sceneitem_t *item, obs_scene_t *parent, bool locked);

/* Only the C-owned calldata and signal boundary; the Rust core decides changes. */
void obs_sceneitem_locked_signal(obs_sceneitem_t *item, obs_scene_t *parent, bool locked)
{
	struct calldata cd;
	uint8_t stack[256];
	calldata_init_fixed(&cd, stack, sizeof(stack));
	calldata_set_ptr(&cd, "item", item);
	calldata_set_bool(&cd, "locked", locked);
	calldata_set_ptr(&cd, "scene", parent);
	signal_handler_signal(parent->source->context.signals, "item_locked", &cd);
}
