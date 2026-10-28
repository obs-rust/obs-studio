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

#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* What the display capture should do when the display configuration changed
 * and the capture re-resolves the uuid it saved. */
enum display_reinit_action {
	/* The saved display is present and its stream is still running: the
	 * reconfiguration did not stop this capture. */
	DISPLAY_REINIT_KEEP = 0,
	/* Nothing can be rebuilt yet: no saved uuid, no enumerated displays,
	 * the saved display is absent, or more than one present display claims
	 * its identity. Retry later with the same saved uuid. */
	DISPLAY_REINIT_WAIT,
	/* The saved display is back and exactly one present display matches it,
	 * but the stream is not running (it stopped when the display went
	 * away): rebuild it. */
	DISPLAY_REINIT_REBUILD,
};

/* Decides what a display capture does after a display reconfiguration, from
 * the uuid it saved in the source settings, the uuids of the displays present
 * now and whether its CGDisplayStream is still running. The saved uuid is
 * never replaced: a display that is gone is waited for, not swapped for a
 * different one (obs-rust/obs-studio#2838). */
enum display_reinit_action display_reinit_decide(const char *stored_uuid, const char *const *present_uuids,
						 size_t count, bool stream_running);

/* The action as it appears in the log line, never NULL. */
const char *display_reinit_action_name(enum display_reinit_action action);

#ifdef __cplusplus
}
#endif
