/* reconnect-policy.c
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 2 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-2.0-or-later
 */

#include "reconnect-policy.h"

#include <errno.h>

#include <pipewire/core.h>

bool obs_pipewire_core_error_is_lost(bool *disconnected, uint32_t id, int res)
{
	/* -EPIPE on the core means the connection to the PipeWire daemon is
	 * gone (e.g. the daemon restarted). It never comes back by itself. */
	if (id != PW_ID_CORE || res != -EPIPE || *disconnected)
		return false;

	*disconnected = true;
	return true;
}

bool obs_pipewire_stream_state_is_lost(bool *disconnected, bool *streamed, enum pw_stream_state state)
{
	if (state == PW_STREAM_STATE_STREAMING)
		*streamed = true;

	/* A stream that failed after streaming does not recover by itself
	 * either (e.g. the producer renegotiated to formats we no longer
	 * accept). Report it like a lost connection so the owner can set
	 * everything up again. Errors before that (no matching format, no
	 * target node) would fail the same way again, so leave those alone. */
	if (state != PW_STREAM_STATE_ERROR || !*streamed || *disconnected)
		return false;

	*disconnected = true;
	return true;
}

bool obs_pipewire_node_removed_is_lost(bool *disconnected, uint32_t node, uint32_t removed_id)
{
	/* The compositor removes a screencast's node when it ends the
	 * screencast, e.g. because its monitor was switched off or unplugged.
	 * The stream connected to it only pauses and never gets another
	 * frame: a new screencast is a new node, handed out by a new portal
	 * session. */
	if (node == PW_ID_ANY || removed_id != node || *disconnected)
		return false;

	*disconnected = true;
	return true;
}

uint32_t obs_pipewire_reconnect_next_delay(uint32_t *attempts)
{
	uint32_t shift;
	uint32_t delay_ms;

	if (*attempts < UINT32_MAX)
		++*attempts;

	/* Never give up: whatever ended the screencast (a monitor that is
	 * off, a portal or PipeWire that is restarting) can come back at any
	 * time. A user who cancels the portal's dialog ends the retries. */
	if (*attempts >= OBS_PIPEWIRE_RECONNECT_FAST_ATTEMPTS)
		return OBS_PIPEWIRE_RECONNECT_SLOW_DELAY_MS;

	shift = *attempts < 6 ? *attempts : 6;
	delay_ms = 250u << shift;
	if (delay_ms > OBS_PIPEWIRE_RECONNECT_MAX_DELAY_MS)
		delay_ms = OBS_PIPEWIRE_RECONNECT_MAX_DELAY_MS;
	return delay_ms;
}
