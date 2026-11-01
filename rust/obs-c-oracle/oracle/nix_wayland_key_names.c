/* Test-only oracle: the original libobs/obs-nix-wayland-key-names.c.h,
 * unmodified, with its function renamed to oracle_* so it can link next to
 * the Rust shim. It calls nothing outside the file. */
#include <stddef.h>
#include <stdint.h>

#define obs_nix_wayland_key_names_route oracle_obs_nix_wayland_key_names_route

#include "obs-nix-wayland-key-names.c.h"

/* The values the C uses, in the order of
 * rust/obs-c-oracle/src/nix_wayland_key_names.rs. */
enum {
	ORACLE_WAYLAND_KEY_NAMES_VALUE_COUNT = 7,
};

int64_t oracle_wayland_key_names_value(size_t which);

int64_t oracle_wayland_key_names_value(size_t which)
{
	const int64_t values[ORACLE_WAYLAND_KEY_NAMES_VALUE_COUNT] = {
		OBS_KEY_LAST_VALUE,
		OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NONE,
		OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_MOUSE,
		OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_NUMPAD,
		OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FIXED,
		OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_FUNCTION,
		OBS_NIX_WAYLAND_KEY_NAMES_ROUTE_KEYSYM,
	};
	return which < ORACLE_WAYLAND_KEY_NAMES_VALUE_COUNT ? values[which] : -1;
}
