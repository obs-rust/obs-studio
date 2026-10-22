/* Test-only oracle: the original libobs/obs-hotkey-press.c.h, unmodified,
 * with its three entry points renamed to oracle_* so they can link next to
 * the Rust shims. The functions it calls keep their real names: the binding
 * accessor obs_hotkey_binding_fields(), the key state
 * obs_hotkey_key_is_pressed() and the press and release wrappers
 * obs_hotkey_binding_press() and obs_hotkey_binding_release(). Under cargo
 * test the test binary defines them over a fake bindings array and logs
 * every call, for this oracle and the shims alike. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
#include <stddef.h>
#include <stdint.h>

#define obs_hotkey_binding_query oracle_obs_hotkey_binding_query
#define obs_hotkey_binding_inject oracle_obs_hotkey_binding_inject
#define obs_hotkeys_query_modifiers oracle_obs_hotkeys_query_modifiers

#include "obs-hotkey-press.c.h"

/* The values and layouts the C uses, in the order of
 * rust/obs-c-oracle/src/hotkey_press.rs. */
enum {
	ORACLE_HOTKEY_PRESS_VALUE_COUNT = 9,
	ORACLE_HOTKEY_PRESS_LAYOUT_COUNT = 9,
};

int64_t oracle_hotkey_press_value(size_t which);
size_t oracle_hotkey_press_layout(size_t which);

int64_t oracle_hotkey_press_value(size_t which)
{
	const int64_t values[ORACLE_HOTKEY_PRESS_VALUE_COUNT] = {
		OBS_KEY_NONE,       OBS_KEY_SHIFT,        OBS_KEY_CONTROL,  OBS_KEY_ALT,          OBS_KEY_META,
		INTERACT_SHIFT_KEY, INTERACT_CONTROL_KEY, INTERACT_ALT_KEY, INTERACT_COMMAND_KEY,
	};
	return which < ORACLE_HOTKEY_PRESS_VALUE_COUNT ? values[which] : -1;
}

size_t oracle_hotkey_press_layout(size_t which)
{
	const size_t values[ORACLE_HOTKEY_PRESS_LAYOUT_COUNT] = {
		sizeof(struct obs_hotkey_binding_fields),
		_Alignof(struct obs_hotkey_binding_fields),
		offsetof(struct obs_hotkey_binding_fields, key),
		offsetof(struct obs_hotkey_binding_fields, pressed),
		offsetof(struct obs_hotkey_binding_fields, modifiers_match),
		sizeof(obs_key_combination_t),
		_Alignof(obs_key_combination_t),
		offsetof(obs_key_combination_t, modifiers),
		offsetof(obs_key_combination_t, key),
	};
	return which < ORACLE_HOTKEY_PRESS_LAYOUT_COUNT ? values[which] : 0;
}
