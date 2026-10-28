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

#include "plugins/mac-capture/build/mac-capture-display-reinit.pch.h"

enum display_reinit_action display_reinit_decide(const char *stored_uuid, const char *const *present_uuids,
						 size_t count, bool stream_running)
{
	/* Nothing to re-resolve: the settings have no uuid to key on, or the
	 * display list is not readable yet while the configuration settles. */
	if (!stored_uuid || !*stored_uuid || !present_uuids || count == 0) {
		return DISPLAY_REINIT_WAIT;
	}

	size_t matches = 0;
	for (size_t i = 0; i < count; i++) {
		if (present_uuids[i] && strcmp(present_uuids[i], stored_uuid) == 0) {
			matches++;
		}
	}

	/* The saved display is gone, or more than one present display claims its
	 * identity: wait for the one this source was created for instead of
	 * switching to another display. */
	if (matches != 1) {
		return DISPLAY_REINIT_WAIT;
	}

	if (stream_running) {
		return DISPLAY_REINIT_KEEP;
	}

	return DISPLAY_REINIT_REBUILD;
}

const char *display_reinit_action_name(enum display_reinit_action action)
{
	switch (action) {
	case DISPLAY_REINIT_KEEP:
		return "keep";
	case DISPLAY_REINIT_WAIT:
		return "wait";
	case DISPLAY_REINIT_REBUILD:
		return "rebuild";
	}

	return "unknown";
}
