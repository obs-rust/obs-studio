/* Test-only oracle: the original libobs/obs-nix-portal-shortcut-ids.c.h,
 * unmodified, with its functions renamed to oracle_* so they can link next
 * to the Rust shims. The functions it calls keep their real names: the
 * dstr functions and obs_key_to_name / obs_key_from_name come from their
 * Rust ports (obs-util, obs-hotkey) under cargo test, as from libobs when
 * this file is not linked. Each of those has its own parity test.
 * bindable() is the header's static inline, wrapped below so the parity
 * test can call it. */
#include <stddef.h>
#include <stdint.h>

#define obs_nix_portal_shortcut_id oracle_obs_nix_portal_shortcut_id
#define obs_nix_portal_shortcut_combo oracle_obs_nix_portal_shortcut_combo
#define obs_nix_portal_shortcut_trigger oracle_obs_nix_portal_shortcut_trigger

/* The codec stores a pointer difference in a size_t; the oracle builds with
 * -Wsign-conversion, libobs does not. */
#if defined(__clang__) || defined(__GNUC__)
#pragma GCC diagnostic ignored "-Wsign-conversion"
#endif

#include "obs-nix-portal-shortcut-ids.c.h"

bool oracle_portal_bindable(obs_key_combination_t combo);

bool oracle_portal_bindable(obs_key_combination_t combo)
{
	return bindable(combo);
}

/* The values the codec uses, in the order of
 * rust/obs-c-oracle/src/portal_shortcut_ids.rs. */
enum {
	ORACLE_PORTAL_VALUE_COUNT = 8,
};

int64_t oracle_portal_value(size_t which);

int64_t oracle_portal_value(size_t which)
{
	const int64_t values[ORACLE_PORTAL_VALUE_COUNT] = {
		OBS_KEY_NONE,         OBS_KEY_LAST_VALUE, OBS_KEY_MOUSE1,     OBS_KEY_MOUSE29,
		INTERACT_CONTROL_KEY, INTERACT_ALT_KEY,   INTERACT_SHIFT_KEY, INTERACT_COMMAND_KEY,
	};
	return which < ORACLE_PORTAL_VALUE_COUNT ? values[which] : -1;
}

/* Entry `which` of the file's modifier_names table; false past its end. */
bool oracle_portal_modifier(size_t which, uint32_t *modifier, const char **id, const char **trigger);

bool oracle_portal_modifier(size_t which, uint32_t *modifier, const char **id, const char **trigger)
{
	if (which >= sizeof(modifier_names) / sizeof(modifier_names[0]))
		return false;
	*modifier = modifier_names[which].modifier;
	*id = modifier_names[which].id;
	*trigger = modifier_names[which].trigger;
	return true;
}
