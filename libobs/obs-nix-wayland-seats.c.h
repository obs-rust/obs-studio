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

/* The seat decisions of the Wayland hotkey platform
 * (obs-nix-wayland-seats.h), moved out of obs-nix-wayland.c. */

#include "libobs/build/libobs-ported-nix-wayland-seats.pch.h"

/* Ported to rust/obs-hotkey (nix_wayland_seats): the hybrid libobs drops
 * this file. */

uint32_t obs_nix_wayland_seats_bind_version(uint32_t offered)
{
	if (offered < 4)
		return 0;
	// Only negotiate up to version 7, the current wl_seat at time of writing.
	return offered <= 7 ? offered : 7;
}

/* A compositor may announce several seats. Take the keyboard from a seat
 * that reports one: asking any other seat for it is a fatal
 * missing_capability protocol error. */
size_t obs_nix_wayland_seats_pick(const struct obs_nix_wayland_seat *seats, size_t num)
{
	for (size_t i = 0; i < num; i++) {
		if (seats[i].capabilities & OBS_NIX_WAYLAND_SEATS_CAPABILITY_KEYBOARD)
			return i;
	}
	return SIZE_MAX;
}

bool obs_nix_wayland_seats_lacks_keyboard(uint32_t capabilities)
{
	return !(capabilities & OBS_NIX_WAYLAND_SEATS_CAPABILITY_KEYBOARD);
}

size_t obs_nix_wayland_seats_find_name(const struct obs_nix_wayland_seat *seats, size_t num, uint32_t name)
{
	for (size_t i = 0; i < num; i++) {
		if (seats[i].name == name)
			return i;
	}
	return SIZE_MAX;
}

bool obs_nix_wayland_seats_uses_release(uint32_t version)
{
	return version >= OBS_NIX_WAYLAND_SEATS_RELEASE_SINCE_VERSION;
}
