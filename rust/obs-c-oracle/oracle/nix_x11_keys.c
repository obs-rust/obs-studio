/* Test-only oracle: the original libobs/obs-nix-x11-keys.c.h, unmodified,
 * with its six functions renamed to oracle_* so they can link next to the
 * Rust shims. It calls nothing outside the file. */
#include <stddef.h>
#include <stdint.h>

#define obs_nix_x11_keys_from_base_keysym oracle_obs_nix_x11_keys_from_base_keysym
#define obs_nix_x11_keys_row oracle_obs_nix_x11_keys_row
#define obs_nix_x11_keys_pressed oracle_obs_nix_x11_keys_pressed
#define obs_nix_x11_keys_from_keycode oracle_obs_nix_x11_keys_from_keycode
#define obs_nix_x11_keys_from_virtual_key oracle_obs_nix_x11_keys_from_virtual_key
#define obs_nix_x11_keys_to_virtual_key oracle_obs_nix_x11_keys_to_virtual_key

#include "obs-nix-x11-keys.c.h"

/* The values and layouts the C uses, in the order of
 * rust/obs-c-oracle/src/nix_x11_keys.rs. */
enum {
	ORACLE_X11_KEYS_VALUE_COUNT = 15,
	ORACLE_X11_KEYS_LAYOUT_COUNT = 5,
};

int64_t oracle_x11_keys_value(size_t which);
size_t oracle_x11_keys_layout(size_t which);

int64_t oracle_x11_keys_value(size_t which)
{
	const int64_t values[ORACLE_X11_KEYS_VALUE_COUNT] = {
		OBS_KEY_LAST_VALUE,
		OBS_KEY_NONE,
		OBS_KEY_SHIFT,
		OBS_KEY_CONTROL,
		OBS_KEY_ALT,
		OBS_KEY_META,
		OBS_NIX_X11_KEYS_SHIFT_R,
		OBS_NIX_X11_KEYS_CONTROL_R,
		OBS_NIX_X11_KEYS_ALT_R,
		OBS_NIX_X11_KEYS_SUPER_L,
		OBS_NIX_X11_KEYS_SUPER_R,
		OBS_NIX_X11_KEYS_ROW_NONE,
		OBS_NIX_X11_KEYS_ROW_SUPER_L,
		OBS_NIX_X11_KEYS_ROW_SUPER_R,
		OBS_NIX_X11_KEYS_ROW_KEY,
	};
	return which < ORACLE_X11_KEYS_VALUE_COUNT ? values[which] : -1;
}

size_t oracle_x11_keys_layout(size_t which)
{
	const size_t values[ORACLE_X11_KEYS_LAYOUT_COUNT] = {
		sizeof(struct keycode_list),
		_Alignof(struct keycode_list),
		offsetof(struct keycode_list, list.array),
		offsetof(struct keycode_list, list.num),
		offsetof(struct keycode_list, list.capacity),
	};
	return which < ORACLE_X11_KEYS_LAYOUT_COUNT ? values[which] : 0;
}
