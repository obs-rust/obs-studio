/* Original extracted C, unchanged and renamed beside the Rust shims. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
#include <stddef.h>
#define obs_hotkey_binding_get_key_combination oracle_obs_hotkey_binding_get_key_combination
#define obs_hotkey_binding_get_hotkey_id oracle_obs_hotkey_binding_get_hotkey_id
#define obs_hotkey_binding_get_hotkey oracle_obs_hotkey_binding_get_hotkey
#include "obs-hotkey-binding-getters.c.h"

size_t oracle_hotkey_binding_getters_layout(size_t which);
size_t oracle_hotkey_binding_getters_layout(size_t which)
{
	const size_t values[] = {
		sizeof(struct obs_hotkey_binding),
		_Alignof(struct obs_hotkey_binding),
		offsetof(struct obs_hotkey_binding, key),
		offsetof(struct obs_hotkey_binding, pressed),
		offsetof(struct obs_hotkey_binding, modifiers_match),
		offsetof(struct obs_hotkey_binding, hotkey_id),
		offsetof(struct obs_hotkey_binding, hotkey),
		sizeof(obs_key_combination_t),
		_Alignof(obs_key_combination_t),
		offsetof(obs_key_combination_t, modifiers),
		offsetof(obs_key_combination_t, key),
	};
	return which < sizeof(values) / sizeof(values[0]) ? values[which] : 0;
}
