/* Test-only oracle: the original libobs/obs-nix-x11-hotkeys.c.h, unmodified,
 * with its two functions renamed to oracle_* so they can link next to the
 * Rust shims. It calls nothing outside the file. */
#include <stddef.h>
#include <stdint.h>

#define obs_nix_x11_hotkeys_route oracle_obs_nix_x11_hotkeys_route
#define obs_nix_x11_hotkeys_raw_button oracle_obs_nix_x11_hotkeys_raw_button

#include "obs-nix-x11-hotkeys.c.h"

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/nix_x11_hotkeys.rs. */
enum {
	ORACLE_X11_HOTKEYS_VALUE_COUNT = 5,
};

int64_t oracle_x11_hotkeys_value(size_t which);

int64_t oracle_x11_hotkeys_value(size_t which)
{
	const int64_t values[ORACLE_X11_HOTKEYS_VALUE_COUNT] = {
		OBS_KEY_LAST_VALUE,
		XINPUT_MOUSE_LEN,
		OBS_NIX_X11_HOTKEYS_GE_GENERIC,
		OBS_NIX_X11_HOTKEYS_RAW_BUTTON_PRESS,
		OBS_NIX_X11_HOTKEYS_RAW_BUTTON_RELEASE,
	};
	return which < ORACLE_X11_HOTKEYS_VALUE_COUNT ? values[which] : -1;
}
