/* Test-only oracle: the original libobs/obs-hotkey-bindings.c.h,
 * unmodified, with its three functions renamed to oracle_* so they can link
 * next to the Rust shims. The functions it calls keep their real names: the
 * obs_data getters and setters and obs_key_from_name / obs_key_to_name come
 * from their Rust ports (obs-data, obs-hotkey) under cargo test, as from
 * libobs when this file is not linked. Each of those has its own parity
 * test, and both sides of this one read and write the same obs_data. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
#include <stdint.h>

#define obs_hotkey_combination_from_data oracle_obs_hotkey_combination_from_data
#define obs_hotkey_combination_bindable oracle_obs_hotkey_combination_bindable
#define obs_hotkey_combination_to_data oracle_obs_hotkey_combination_to_data

#include "obs-hotkey-bindings.c.h"

/* The values the codec uses, in the order of
 * rust/obs-c-oracle/src/hotkey_bindings.rs. */
enum {
	ORACLE_HOTKEY_BINDINGS_VALUE_COUNT = 8,
};

int64_t oracle_hotkey_bindings_value(size_t which);

int64_t oracle_hotkey_bindings_value(size_t which)
{
	const int64_t values[ORACLE_HOTKEY_BINDINGS_VALUE_COUNT] = {
		OBS_KEY_NONE,       OBS_KEY_LAST_VALUE,   OBS_KEY_MOUSE1,   OBS_KEY_MOUSE2,
		INTERACT_SHIFT_KEY, INTERACT_CONTROL_KEY, INTERACT_ALT_KEY, INTERACT_COMMAND_KEY,
	};
	return which < ORACLE_HOTKEY_BINDINGS_VALUE_COUNT ? values[which] : -1;
}
