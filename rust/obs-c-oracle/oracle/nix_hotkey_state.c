/* Test-only oracle: the original libobs/obs-nix-hotkey-state.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims. obs_hotkeys_mutex_lock/unlock and bmalloc stay the
 * real symbols; cargo tests provide the lock pair (counted), so the C and
 * the Rust lock the same way. libobs provides them when this file is not
 * linked. */
#include <stddef.h>
#include <stdint.h>

#define obs_nix_hotkey_state_free oracle_obs_nix_hotkey_state_free
#define obs_nix_hotkey_state_set_key oracle_obs_nix_hotkey_state_set_key
#define obs_nix_hotkey_state_set_modifiers oracle_obs_nix_hotkey_state_set_modifiers
#define obs_nix_hotkey_state_clear_keys oracle_obs_nix_hotkey_state_clear_keys
#define obs_nix_hotkey_state_set_shortcut oracle_obs_nix_hotkey_state_set_shortcut
#define obs_nix_hotkey_state_flush oracle_obs_nix_hotkey_state_flush
#define obs_nix_hotkey_state_is_pressed oracle_obs_nix_hotkey_state_is_pressed

#include "obs-nix-hotkey-state.c.h"

/* The values and layouts the C uses, in the order of
 * rust/obs-c-oracle/src/nix_hotkey_state.rs. */
enum {
	ORACLE_HOTKEY_STATE_VALUE_COUNT = 9,
	ORACLE_HOTKEY_STATE_LAYOUT_COUNT = 13,
};

int64_t oracle_hotkey_state_value(size_t which);
size_t oracle_hotkey_state_layout(size_t which);

int64_t oracle_hotkey_state_value(size_t which)
{
	const int64_t values[ORACLE_HOTKEY_STATE_VALUE_COUNT] = {
		OBS_KEY_LAST_VALUE, OBS_KEY_SHIFT,        OBS_KEY_CONTROL,  OBS_KEY_ALT,          OBS_KEY_META,
		INTERACT_SHIFT_KEY, INTERACT_CONTROL_KEY, INTERACT_ALT_KEY, INTERACT_COMMAND_KEY,
	};
	return which < ORACLE_HOTKEY_STATE_VALUE_COUNT ? values[which] : -1;
}

size_t oracle_hotkey_state_layout(size_t which)
{
	const size_t values[ORACLE_HOTKEY_STATE_LAYOUT_COUNT] = {
		sizeof(obs_key_combination_t),
		_Alignof(obs_key_combination_t),
		offsetof(obs_key_combination_t, modifiers),
		offsetof(obs_key_combination_t, key),
		sizeof(struct obs_nix_hotkey_shortcut),
		_Alignof(struct obs_nix_hotkey_shortcut),
		offsetof(struct obs_nix_hotkey_shortcut, combo),
		offsetof(struct obs_nix_hotkey_shortcut, observed),
		offsetof(struct obs_nix_hotkey_shortcut, release_pending),
		sizeof(struct obs_nix_hotkey_state),
		_Alignof(struct obs_nix_hotkey_state),
		offsetof(struct obs_nix_hotkey_state, focused_modifiers),
		offsetof(struct obs_nix_hotkey_state, shortcuts),
	};
	return which < ORACLE_HOTKEY_STATE_LAYOUT_COUNT ? values[which] : 0;
}
