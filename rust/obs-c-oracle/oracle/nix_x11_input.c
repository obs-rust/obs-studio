/* Test-only oracle: the original libobs/obs-nix-x11-input.c.h, unmodified,
 * with its four functions renamed to oracle_* so they can link next to the
 * Rust shims. It calls nothing outside the file. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define obs_nix_x11_input_keysym oracle_obs_nix_x11_input_keysym
#define obs_nix_x11_input_mouse_pressed oracle_obs_nix_x11_input_mouse_pressed
#define obs_nix_x11_input_commit oracle_obs_nix_x11_input_commit
#define obs_nix_x11_input_mask_pressed oracle_obs_nix_x11_input_mask_pressed

#include "obs-nix-x11-input.c.h"

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/nix_x11_input.rs. */
enum {
	ORACLE_X11_INPUT_VALUE_COUNT = 9,
};

int64_t oracle_x11_input_value(size_t which);

int64_t oracle_x11_input_value(size_t which)
{
	const int64_t values[ORACLE_X11_INPUT_VALUE_COUNT] = {
		OBS_KEY_LAST_VALUE,
		OBS_KEY_NONE,
		OBS_KEY_MOUSE1,
		OBS_KEY_MOUSE29,
		XINPUT_MOUSE_LEN,
		OBS_NIX_X11_INPUT_BUTTON_MASK_1,
		OBS_NIX_X11_INPUT_BUTTON_MASK_2,
		OBS_NIX_X11_INPUT_BUTTON_MASK_3,
		sizeof(bool),
	};
	return which < ORACLE_X11_INPUT_VALUE_COUNT ? values[which] : -1;
}
