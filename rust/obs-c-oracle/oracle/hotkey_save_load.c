/* Test-only oracle: the original libobs/obs-hotkey-save-load.c.h, unmodified,
 * with its functions renamed to oracle_* so they can link next to the Rust
 * shims. The functions it calls keep their real names: the private seam
 * (obs-hotkey-seam.h) and the public hotkey getters, which the test binary
 * defines over a fake registry and logs for this oracle and the shims alike,
 * and the obs_data functions and the binding codec, which come from their Rust
 * ports under cargo test. Each of those has its own parity test. */
#ifndef _WIN32
#define _GNU_SOURCE
#endif
#include <stddef.h>
#include <stdint.h>

#define load_bindings oracle_load_bindings
#define obs_hotkey_load_bindings oracle_obs_hotkey_load_bindings
#define obs_hotkey_load oracle_obs_hotkey_load
#define obs_hotkeys_load_encoder oracle_obs_hotkeys_load_encoder
#define obs_hotkeys_load_output oracle_obs_hotkeys_load_output
#define obs_hotkeys_load_service oracle_obs_hotkeys_load_service
#define obs_hotkeys_load_source oracle_obs_hotkeys_load_source
#define obs_hotkey_pair_load oracle_obs_hotkey_pair_load
#define obs_hotkey_save oracle_obs_hotkey_save
#define obs_hotkey_pair_save oracle_obs_hotkey_pair_save
#define obs_hotkeys_save_encoder oracle_obs_hotkeys_save_encoder
#define obs_hotkeys_save_output oracle_obs_hotkeys_save_output
#define obs_hotkeys_save_service oracle_obs_hotkeys_save_service
#define obs_hotkeys_save_source oracle_obs_hotkeys_save_source

#include "obs-hotkey-save-load.c.h"

/* The values the file uses, in the order of
 * rust/obs-c-oracle/src/hotkey_save_load.rs. */
enum {
	ORACLE_HOTKEY_SAVE_LOAD_VALUE_COUNT = 2,
};

int64_t oracle_hotkey_save_load_value(size_t which);

int64_t oracle_hotkey_save_load_value(size_t which)
{
	const int64_t values[ORACLE_HOTKEY_SAVE_LOAD_VALUE_COUNT] = {
		OBS_KEY_VK_VOLUME_UP,
		OBS_KEY_VK_VOLUME_DOWN,
	};
	return which < ORACLE_HOTKEY_SAVE_LOAD_VALUE_COUNT ? values[which] : 0;
}
