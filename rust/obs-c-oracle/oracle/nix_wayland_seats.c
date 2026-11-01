/* Test-only oracle: the original libobs/obs-nix-wayland-seats.c.h,
 * unmodified, with its five functions renamed to oracle_* so they can link
 * next to the Rust shims. It calls nothing outside the file and needs no
 * Wayland header. */
#include <stddef.h>
#include <stdint.h>

#define obs_nix_wayland_seats_bind_version oracle_obs_nix_wayland_seats_bind_version
#define obs_nix_wayland_seats_pick oracle_obs_nix_wayland_seats_pick
#define obs_nix_wayland_seats_lacks_keyboard oracle_obs_nix_wayland_seats_lacks_keyboard
#define obs_nix_wayland_seats_find_name oracle_obs_nix_wayland_seats_find_name
#define obs_nix_wayland_seats_uses_release oracle_obs_nix_wayland_seats_uses_release

#include "obs-nix-wayland-seats.c.h"

/* The values and the seat layout the C uses, in the order of
 * rust/obs-c-oracle/src/nix_wayland_seats.rs. */
enum {
	ORACLE_WAYLAND_SEATS_VALUE_COUNT = 7,
};

int64_t oracle_wayland_seats_value(size_t which);

/* The seat's alignment, without C11's _Alignof (MSVC builds this too). */
struct oracle_wayland_seat_align {
	char c;
	struct obs_nix_wayland_seat seat;
};

int64_t oracle_wayland_seats_value(size_t which)
{
	const int64_t values[ORACLE_WAYLAND_SEATS_VALUE_COUNT] = {
		OBS_NIX_WAYLAND_SEATS_CAPABILITY_KEYBOARD,
		OBS_NIX_WAYLAND_SEATS_RELEASE_SINCE_VERSION,
		(int64_t)sizeof(struct obs_nix_wayland_seat),
		(int64_t)offsetof(struct oracle_wayland_seat_align, seat),
		(int64_t)offsetof(struct obs_nix_wayland_seat, seat),
		(int64_t)offsetof(struct obs_nix_wayland_seat, name),
		(int64_t)offsetof(struct obs_nix_wayland_seat, capabilities),
	};
	return which < ORACLE_WAYLAND_SEATS_VALUE_COUNT ? values[which] : -1;
}
