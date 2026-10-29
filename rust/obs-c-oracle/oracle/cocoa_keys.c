/* Test-only oracle: the original libobs/obs-cocoa-keys.c.h, unmodified,
 * with its six functions renamed to oracle_* so they can link next to the
 * Rust shims. It calls nothing outside the file. */
#include <stddef.h>
#include <stdint.h>

#define obs_cocoa_keys_modifier oracle_obs_cocoa_keys_modifier
#define obs_cocoa_keys_to_virtual_key oracle_obs_cocoa_keys_to_virtual_key
#define obs_cocoa_keys_from_virtual_key oracle_obs_cocoa_keys_from_virtual_key
#define obs_cocoa_keys_is_mouse oracle_obs_cocoa_keys_is_mouse
#define obs_cocoa_keys_mouse_pressed oracle_obs_cocoa_keys_mouse_pressed
#define obs_cocoa_keys_in_range oracle_obs_cocoa_keys_in_range

#include "obs-cocoa-keys.c.h"

/* The values and layouts the C uses, in the order of
 * rust/obs-c-oracle/src/cocoa_keys.rs. */
enum {
	ORACLE_COCOA_KEYS_VALUE_COUNT = 13,
	ORACLE_COCOA_KEYS_LAYOUT_COUNT = 4,
};

int64_t oracle_cocoa_keys_value(size_t which);
size_t oracle_cocoa_keys_layout(size_t which);

int64_t oracle_cocoa_keys_value(size_t which)
{
	const int64_t values[ORACLE_COCOA_KEYS_VALUE_COUNT] = {
		OBS_KEY_LAST_VALUE,
		OBS_KEY_NONE,
		OBS_KEY_SHIFT,
		OBS_KEY_CONTROL,
		OBS_KEY_ALT,
		OBS_KEY_META,
		OBS_KEY_MOUSE1,
		OBS_KEY_MOUSE29,
		OBS_COCOA_KEYS_FLAG_SHIFT,
		OBS_COCOA_KEYS_FLAG_CONTROL,
		OBS_COCOA_KEYS_FLAG_ALTERNATE,
		OBS_COCOA_KEYS_FLAG_COMMAND,
		OBS_COCOA_KEYS_INVALID,
	};
	return which < ORACLE_COCOA_KEYS_VALUE_COUNT ? values[which] : -1;
}

size_t oracle_cocoa_keys_layout(size_t which)
{
	const size_t values[ORACLE_COCOA_KEYS_LAYOUT_COUNT] = {
		sizeof(obs_key_code_t),
		_Alignof(obs_key_code_t),
		offsetof(obs_key_code_t, code),
		offsetof(obs_key_code_t, is_valid),
	};
	return which < ORACLE_COCOA_KEYS_LAYOUT_COUNT ? values[which] : 0;
}
