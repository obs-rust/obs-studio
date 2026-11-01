/******************************************************************************
    Copyright (C) 2019 by Jason Francis <cycl0ps@tuta.io>

    This program is free software: you can redistribute it and/or modify
    it under the terms of the GNU General Public License as published by
    the Free Software Foundation, either version 2 of the License, or
    (at your option) any later version.

    This program is distributed in the hope that it will be useful,
    but WITHOUT ANY WARRANTY; without even the implied warranty of
    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
    GNU General Public License for more details.

    You should have received a copy of the GNU General Public License
    along with this program.  If not, see <http://www.gnu.org/licenses/>.
******************************************************************************/

#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

/* The seat decisions of the Wayland hotkey platform, on plain values and
 * the platform's seat array (obs-nix-wayland-seats.c):
 * - obs_nix_wayland_seats_bind_version(): platform_registry_handler()'s
 *   wl_seat version;
 * - obs_nix_wayland_seats_pick(): the seat take_keyboard() takes the
 *   keyboard from;
 * - obs_nix_wayland_seats_lacks_keyboard(): whether a seat
 *   platform_seat_capabilities() hears from has no keyboard (the release
 *   then also needs it to be the keyboard's seat, which obs-nix-wayland.c
 *   compares only after this);
 * - obs_nix_wayland_seats_find_name(): the seat platform_registry_remover()
 *   drops;
 * - obs_nix_wayland_seats_uses_release(): release_seat()'s wl_seat_release
 *   or wl_seat_destroy.
 *
 * obs-nix-wayland.c keeps every Wayland call, the listeners, the log line,
 * the seat array's pushes and erases, find_seat(), release_keyboard() and
 * the order of the steps in each handler. Nothing here calls into Wayland,
 * allocates, logs or keeps state. Without EXPORT, so hidden in libobs.
 *
 * obs-nix-wayland.c checks the two Wayland values below against
 * <wayland-client-protocol.h>, and the seat's layout. */

/* WL_SEAT_CAPABILITY_KEYBOARD: the seat has a keyboard. */
#define OBS_NIX_WAYLAND_SEATS_CAPABILITY_KEYBOARD 2
/* WL_SEAT_RELEASE_SINCE_VERSION: the first wl_seat with wl_seat_release. */
#define OBS_NIX_WAYLAND_SEATS_RELEASE_SINCE_VERSION 5

struct wl_seat;

/* A seat the compositor announced: the bound wl_seat (never read here), its
 * registry name and its last reported capabilities. */
struct obs_nix_wayland_seat {
	struct wl_seat *seat;
	uint32_t name;
	uint32_t capabilities;
};

/* The version to bind a wl_seat the compositor offers at version offered:
 * 0 below version 4 (too old: no hotkeys from it), else offered, at most 7. */
uint32_t obs_nix_wayland_seats_bind_version(uint32_t offered);

/* The index of the first of the num seats whose capabilities have
 * OBS_NIX_WAYLAND_SEATS_CAPABILITY_KEYBOARD, SIZE_MAX for none. Only the
 * capabilities are read, in order, up to that seat. */
size_t obs_nix_wayland_seats_pick(const struct obs_nix_wayland_seat *seats, size_t num);

/* Whether capabilities lack OBS_NIX_WAYLAND_SEATS_CAPABILITY_KEYBOARD: the
 * seat reporting them has no keyboard. */
bool obs_nix_wayland_seats_lacks_keyboard(uint32_t capabilities);

/* The index of the first of the num seats with registry name name, SIZE_MAX
 * for none. Only the names are read, in order, up to that seat. */
size_t obs_nix_wayland_seats_find_name(const struct obs_nix_wayland_seat *seats, size_t num, uint32_t name);

/* Whether a wl_seat bound at version is released with wl_seat_release (from
 * OBS_NIX_WAYLAND_SEATS_RELEASE_SINCE_VERSION on) rather than destroyed. */
bool obs_nix_wayland_seats_uses_release(uint32_t version);
