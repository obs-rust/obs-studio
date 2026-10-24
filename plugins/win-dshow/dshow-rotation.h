/*
 * This program is free software; you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation; either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <http://www.gnu.org/licenses/>.
 */

#pragma once

/* When a Video Capture Device source passes the device's roll to
 * obs_source_set_async_rotation() (win-dshow.cpp.hpp, the Autorotation
 * setting, obs-rust/obs-studio#2137). Plain C with no Windows headers, so a
 * cmocka test drives it on every host. *last_rotation is the rotation the
 * source last applied (DShowInput::lastRotation). */

#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/* A frame arrived with the device's roll. Returns true, and records it, when
 * the caller must apply device_rotation. */
static inline bool dshow_rotation_on_frame(bool autorotation, long *last_rotation, long device_rotation)
{
	if (!autorotation || device_rotation == *last_rotation) {
		return false;
	}

	*last_rotation = device_rotation;
	return true;
}

/* The settings were read. Returns true, and records 0, when the caller must
 * set the rotation back to 0. */
static inline bool dshow_rotation_on_settings(bool autorotation, long *last_rotation)
{
	if (autorotation || *last_rotation == 0) {
		return false;
	}

	/* The next frame with Autorotation on applies the device's roll again,
	 * even when it is the roll applied before. */
	*last_rotation = 0;
	return true;
}

#ifdef __cplusplus
}
#endif
