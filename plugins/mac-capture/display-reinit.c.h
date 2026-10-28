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

enum display_reinit_action display_reinit_decide(const char *stored_uuid, const char *const *present_uuids, size_t count,
						 bool stream_running)
{
	UNUSED_PARAMETER(stored_uuid);
	UNUSED_PARAMETER(present_uuids);
	UNUSED_PARAMETER(count);
	UNUSED_PARAMETER(stream_running);

	/* RED stub (obs-rust/obs-studio#2838): always keeps the stream. */
	return DISPLAY_REINIT_KEEP;
}

const char *display_reinit_action_name(enum display_reinit_action action)
{
	/* RED stub (obs-rust/obs-studio#2838). */
	UNUSED_PARAMETER(action);
	return "keep";
}
